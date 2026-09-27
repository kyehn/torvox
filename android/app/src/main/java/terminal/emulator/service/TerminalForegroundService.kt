package terminal.emulator.service

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import terminal.emulator.MainActivity
import terminal.emulator.R

class TerminalForegroundService : Service() {
    companion object {
        private const val CHANNEL_ID = "terminal"
        private const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_TAG = "termvox:wakelock"

        // 安全网：唤醒锁绝不能活得比它所保活的会话更久。
        // 30 分钟覆盖可预期的最长交互运行；仍存活的会话会在下个 start 节拍重新获取。
        private const val WAKE_LOCK_TIMEOUT_MS = 30 * 60 * 1000L

        fun start(context: Context) {
            val intent = Intent(context, TerminalForegroundService::class.java)
            context.startForegroundService(intent)
        }

        fun stop(context: Context): Boolean = context.stopService(
            Intent(context, TerminalForegroundService::class.java),
        )

        fun updateSessionCount(context: Context, count: Int) {
            if (count <= 0) {
                // 刻意忽略返回值：对已停止服务 stopService 返回 false
                // 同样是期望的终态。
                stop(context)
                return
            }
            val intent =
                Intent(context, TerminalForegroundService::class.java).apply {
                    putExtra("session_count", count)
                }
            try {
                context.startForegroundService(intent)
            } catch (exception: Exception) {
                // API 31+：应用在后台而服务尚未运行时抛 ForegroundServiceStartNotAllowedException
                // （如系统杀掉了它而 START_STICKY 尚未重启）。
                // 这绝不能使渲染线程崩溃。
                android.util.Log.w("TerminalForegroundService", "startForegroundService failed", exception)
            }
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null
    private var sessionCount: Int = 0

    override fun onCreate() {
        super.onCreate()
        val channel =
            NotificationChannel(
                CHANNEL_ID,
                getString(R.string.notification_channel_name),
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = getString(R.string.notification_channel_desc)
                setShowBadge(false)
            }
        val notificationManager = getSystemService(NotificationManager::class.java)
        notificationManager.createNotificationChannel(channel)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // 进程被杀后 START_STICKY 重启：没有会话能在进程死亡后存活，
        // 故服务（及其 PARTIAL_WAKE_LOCK）已无保活对象。
        // 改为停止，而不是带着永久唤醒锁无休止地重新固定通知。
        if (intent == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        sessionCount = intent.getIntExtra("session_count", 1).coerceAtLeast(1)
        startForegroundWithSessionCount(sessionCount)
        acquireWakeLockIfNeeded()
        return START_STICKY
    }

    // 仅简体中文：无复数形态，plurals 仅 other 分支生效，无需本地化计数修饰。
    @SuppressLint("ArgInFormattedQuantityStringRes")
    private fun startForegroundWithSessionCount(count: Int) {
        val text =
            if (count <= 1) {
                getString(R.string.notification_active_single)
            } else {
                resources.getQuantityString(R.plurals.notification_active_plural, count, count)
            }
        val openIntent =
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            }
        val pending =
            PendingIntent.getActivity(
                this,
                0,
                openIntent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        val notification =
            Notification
                .Builder(this, CHANNEL_ID)
                .setContentTitle(getString(R.string.app_name))
                .setContentText(text)
                .setSmallIcon(R.drawable.ic_notification)
                .setOngoing(true)
                .setContentIntent(pending)
                .setCategory(Notification.CATEGORY_SERVICE)
                .build()
        try {
            startForeground(NOTIFICATION_ID, notification)
        } catch (exception: Exception) {
            // minSdk 33 下缺少 POST_NOTIFICATIONS 权限（以及部分厂商 ROM）
            // 会使主 onStartCommand 路径上的 startForeground 抛 SecurityException
            // ——静态的 updateSessionCount 路径已有守卫；此路径绝不能使进程崩溃。
            //
            // 已知局限：运行期的 foregroundServiceRunning 标志
            // 在此调用之前已被 startForegroundServiceIfNeeded 置真，
            // 且没有任何失败信号回传——后续的 startForegroundServiceIfNeeded
            // 会因（陈旧的）标志而跳过启动，直到计数经 updateForegroundSessionCount
            // 归零或 stopForegroundService 运行。服务本身仍由运行期的 startService
            // 调用所绑定，故唤醒锁与前台进程保证仍然成立；只是缺少通知。
            // 关闭所有会话即可自愈。
            Log.e("TerminalForegroundService", "startForeground failed", exception)
        }
    }

    private fun acquireWakeLockIfNeeded() {
        if (wakeLock?.isHeld == true) return
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock =
            powerManager
                .newWakeLock(
                    PowerManager.PARTIAL_WAKE_LOCK,
                    WAKE_LOCK_TAG,
                ).apply {
                    setReferenceCounted(false)
                    acquire(WAKE_LOCK_TIMEOUT_MS)
                }
    }

    private fun releaseWakeLock() {
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
    }

    override fun onBind(intent: Intent?): IBinder = Binder()

    override fun onDestroy() {
        releaseWakeLock()
        super.onDestroy()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        // 服务在存活的终端会话下继续运行（START_STICKY）。
        // 重新获取唤醒锁而不是丢弃它：否则在任务被划掉且屏幕关闭时，
        // 会话的 CPU 与网络访问会被冻结且无从恢复
        // （此后再无任何调用 acquireWakeLockIfNeeded）。
        if (wakeLock?.isHeld != true) {
            acquireWakeLockIfNeeded()
        }
    }
}
