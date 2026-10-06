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
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import terminal.emulator.MainActivity
import terminal.emulator.R
import terminal.emulator.runtime.LogUtil

class TerminalForegroundService : Service() {
    companion object {
        private const val CHANNEL_ID = "terminal"
        private const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_TAG = "termvox:wakelock"
        private const val EXTRA_SESSION_COUNT = "session_count"

        // 唤醒锁单次持有的上限：安全网，由 [scheduleWakeLockRenewal] 在半程续期。
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
                    putExtra(EXTRA_SESSION_COUNT, count)
                }
            try {
                context.startForegroundService(intent)
            } catch (exception: Exception) {
                // API 31+：应用在后台而服务尚未运行时抛 ForegroundServiceStartNotAllowedException
                // （如系统杀掉了它而 START_STICKY 尚未重启）。
                // 这绝不能使渲染线程崩溃。
                LogUtil.w("TerminalForegroundService", "startForegroundService failed", exception)
            }
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null

    // 续期走主线程 Handler：唤醒锁的释放/重取必须在有 Looper 的线程上完成。
    private val wakeLockRenewal = Handler(Looper.getMainLooper())

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
        sessionCount = if (intent.hasExtra(EXTRA_SESSION_COUNT)) {
            intent.getIntExtra(EXTRA_SESSION_COUNT, 0).coerceAtLeast(0)
        } else {
            // 裸 start()（冷启动/首个会话）不带计数：不得回落成 1——此时可能尚无会话，
            // 谎报「1 个活动会话」并为不存在的会话持有唤醒锁。
            // 沿用上次已知值（冷启动为 0，即「启动中」）。
            sessionCount
        }
        startForegroundWithSessionCount(sessionCount)
        if (sessionCount >= 1) acquireWakeLockIfNeeded()
        return START_STICKY
    }

    // 仅简体中文：无复数形态，plurals 仅 other 分支生效，无需本地化计数修饰。
    @SuppressLint("ArgInFormattedQuantityStringRes")
    private fun startForegroundWithSessionCount(count: Int) {
        val text =
            when {
                count <= 0 -> getString(R.string.notification_starting)
                count == 1 -> getString(R.string.notification_active_single)
                else -> resources.getQuantityString(R.plurals.notification_active_plural, count, count)
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
        // 缺失 POST_NOTIFICATIONS 不会使这里抛异常（平台文档：前台服务照常启动，
        // 只是通知不进抽屉、仅见于任务管理器）；实测 API 35 上拒绝该权限后
        // startForeground 仍成功（dumpsys 报 isForeground=true）。
        // 真正的失败（缺 FOREGROUND_SERVICE 权限、被判为后台启动）按 DESIGN:16
        // 记日志并让进程崩掉，不在此静默吞掉。
        startForeground(NOTIFICATION_ID, notification)
    }

    /**
     * 获取唤醒锁并排定续期。
     *
     * 超时是安全网（进程崩溃或漏释放时不至于永久耗电），续期保证空闲会话不会
     * 因超时而过早失锁：只带超时的旧实现在 30 分钟后锁自动失效，而服务仍在
     * 运行、`onStartCommand` 不会再被触发取回锁，灭屏期间的会话被系统冻结。
     * 续期间隔取超时的一半，主线程被长时间占用时仍留有一次完整补救窗口。
     */
    private fun acquireWakeLockIfNeeded() {
        if (wakeLock?.isHeld == true) {
            scheduleWakeLockRenewal()
            return
        }
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock =
            powerManager
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, WAKE_LOCK_TAG)
                .apply {
                    setReferenceCounted(false)
                    acquire(WAKE_LOCK_TIMEOUT_MS)
                }
        scheduleWakeLockRenewal()
    }

    private fun scheduleWakeLockRenewal() {
        wakeLockRenewal.removeCallbacksAndMessages(null)
        wakeLockRenewal.postDelayed(
            { renewWakeLock() },
            WAKE_LOCK_TIMEOUT_MS / 2,
        )
    }

    private fun renewWakeLock() {
        val lock = wakeLock
        if (lock == null) return
        if (!lock.isHeld) {
            acquireWakeLockIfNeeded()
            return
        }
        lock.release()
        lock.acquire(WAKE_LOCK_TIMEOUT_MS)
        scheduleWakeLockRenewal()
    }

    private fun releaseWakeLock() {
        wakeLockRenewal.removeCallbacksAndMessages(null)
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
        // 零会话时不取：裸 start() 冷启动会走到这里，此时 sessionCount 仍为 0，
        // 为不存在的会话持唤醒锁（同 onStartCommand 的判据）。
        if (sessionCount >= 1 && wakeLock?.isHeld != true) {
            acquireWakeLockIfNeeded()
        }
    }
}
