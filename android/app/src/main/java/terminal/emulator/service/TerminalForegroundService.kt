package terminal.emulator.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
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
        private const val WIFI_LOCK_TAG = "termvox:wifilock"
        private const val EXTRA_SESSION_COUNT = "session_count"
        private const val ACTION_EXIT = "terminal.emulator.action.EXIT"

        // 唤醒锁单次持有的上限：安全网，由 [scheduleWakeLockRenewal] 在半程续期。
        private const val WAKE_LOCK_TIMEOUT_MS = 30 * 60 * 1000L

        fun start(context: Context) {
            val intent = Intent(context, TerminalForegroundService::class.java)
            context.startForegroundService(intent)
        }

        fun stop(context: Context): Boolean = context.stopService(
            Intent(context, TerminalForegroundService::class.java),
        )

        /**
         * 启动或更新前台服务，返回是否真的发起。
         *
         * 返回值必须外传：后台启动被系统拒绝时（API 31+ 的
         * `ForegroundServiceStartNotAllowedException`，如系统杀掉服务而 `START_STICKY`
         * 尚未重启），调用方的「服务在运行」标志若停在真，此后
         * `startForegroundServiceIfNeeded` 会永远跳过重启——存活会话既没有常驻通知
         * 也没有 `PARTIAL_WAKE_LOCK`，后台随时可能被杀且用户看不到任何提示。
         */
        fun updateSessionCount(context: Context, count: Int): Boolean {
            if (count <= 0) {
                // 刻意忽略返回值：对已停止服务 stopService 返回 false
                // 同样是期望的终态。
                stop(context)
                return true
            }
            val intent =
                Intent(context, TerminalForegroundService::class.java).apply {
                    putExtra(EXTRA_SESSION_COUNT, count)
                }
            return try {
                context.startForegroundService(intent)
                true
            } catch (exception: Exception) {
                // 这绝不能使渲染线程崩溃；但失败必须如实报回，不得伪装成已启动。
                LogUtil.warn("TerminalForegroundService", "startForegroundService failed", exception)
                false
            }
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null

    /**
     * Wi-Fi 锁：与唤醒锁配对，参考 termux 的 `WifiLock`（`TermuxService`）。
     *
     * 唤醒锁只保证 CPU 不休眠，Wi-Fi 仍会进入省电模式——长连接（ssh、mosh、下载）
     * 在灭屏几分钟后表现为「卡住，一碰屏幕就恢复」。Wi-Fi 锁把射频钉在高性能档，
     * 代价是这一段确实更费电，故与唤醒锁同生命周期：只有存在会话时才持有。
     */
    private var wifiLock: WifiManager.WifiLock? = null

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
        // 通知的「退出」按钮（DESIGN：常驻通知提供退出按钮）：关闭全部会话后
        // 前台服务计数归零并自行停掉——与侧边面板关闭会话是同一条既有路径。
        if (intent?.action == ACTION_EXIT) {
            dagger.hilt.android.EntryPointAccessors
                .fromApplication(applicationContext, terminal.emulator.runtime.TerminalRuntimeEntryPoint::class.java)
                .terminalRuntime()
                .closeAllSessions()
            stopSelf()
            return START_NOT_STICKY
        }
        // 进程被杀后 START_STICKY 重启：没有会话能在进程死亡后存活，
        // 故服务（及其 PARTIAL_WAKE_LOCK）已无保活对象。
        // 改为停止，而不是带着永久唤醒锁无休止地重新固定通知。
        if (intent == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        sessionCount =
            if (intent.hasExtra(EXTRA_SESSION_COUNT)) {
                intent.getIntExtra(EXTRA_SESSION_COUNT, 0).coerceAtLeast(0)
            } else {
                // 裸 start()（冷启动/首个会话）不带计数：不得回落成 1——此时可能尚无会话，
                // 谎报「1 个活动会话」并为不存在的会话持有唤醒锁。
                // 沿用上次已知值（冷启动为 0，即「启动中」）。
                sessionCount
            }
        startForegroundWithSessionCount(sessionCount)
        if (sessionCount >= 1) {
            acquireWakeLockIfNeeded()
            acquireWifiLockIfNeeded()
        }
        return START_STICKY
    }

    private fun startForegroundWithSessionCount(count: Int) {
        val text =
            when {
                count <= 0 -> getString(R.string.notification_starting)
                count == 1 -> getString(R.string.notification_active_single)
                else -> getString(R.string.notification_active_multiple, count)
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
        val exitPending =
            PendingIntent.getService(
                this,
                1,
                Intent(this, TerminalForegroundService::class.java).setAction(ACTION_EXIT),
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
                .addAction(
                    Notification.Action
                        .Builder(
                            null,
                            getString(R.string.notification_exit),
                            exitPending,
                        ).build(),
                ).build()
        // 缺失 POST_NOTIFICATIONS 不会使这里抛异常（平台文档：前台服务照常启动，
        // 只是通知不进抽屉、仅见于任务管理器）；实测 API 35 上拒绝该权限后
        // startForeground 仍成功（dumpsys 报 isForeground=true）。
        // 真正的失败（缺 FOREGROUND_SERVICE 权限、被判为后台启动）按 DESIGN.md:17
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

    /**
     * 获取 Wi-Fi 锁。
     *
     * 用 `WIFI_MODE_FULL_HIGH_PERF`（与 termux 相同）：该模式在**灭屏、应用退到后台**
     * 时依然生效，而这正是这把锁存在的意义——唤醒锁保证 CPU 不休眠，Wi-Fi 若此时进入
     * 省电，ssh/mosh 这类长连接会「卡住，一碰屏幕就恢复」。
     *
     * `WIFI_MODE_FULL_LOW_LATENCY`（API 29 起）不满足这个前提：官方文档写明它
     * 「只在屏幕点亮且应用位于前台时生效」，在终端最需要它的时候它等于没加。
     *
     * `WIFI_MODE_FULL_HIGH_PERF` 在 API 34 起被标记废弃，平台把这类请求转成
     * `WIFI_MODE_FULL_LOW_LATENCY`（连同它的限制）——即请求它绝不会比直接请求低延迟
     * 模式更差，而在仍按老语义处理的系统上它才是灭屏后仍生效的那一个。termux 用的
     * 也是这个模式。
     *
     * 网络不可用（无 Wi-Fi 硬件、取不到 WifiManager）时静默跳过：终端不依赖 Wi-Fi，
     * 移动数据或本地 shell 照常工作，为它报错只会制造噪声。
     */
    @Suppress("DEPRECATION") // 高性能锁在 API 34 起废弃，但平台会把它转成低延迟锁，不会更差。
    private fun acquireWifiLockIfNeeded() {
        if (wifiLock?.isHeld == true) return
        val wifiManager =
            applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
                ?: return
        wifiLock =
            wifiManager
                .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, WIFI_LOCK_TAG)
                .apply {
                    setReferenceCounted(false)
                    acquire()
                }
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

    /**
     * 服务是 `android:exported="false"` 且全仓无 `bindService` 调用，`onBind` 必须
     * 显式声明为 null——抽象成员不实现连编译都过不去，「无人调用」不是省略的理由。
     */
    override fun onBind(intent: Intent?): IBinder? = null

    private fun releaseWakeLock() {
        wakeLockRenewal.removeCallbacksAndMessages(null)
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
    }

    private fun releaseWifiLock() {
        wifiLock?.takeIf { it.isHeld }?.release()
        wifiLock = null
    }

    override fun onDestroy() {
        releaseWakeLock()
        releaseWifiLock()
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
        if (sessionCount >= 1) {
            if (wakeLock?.isHeld != true) acquireWakeLockIfNeeded()
            acquireWifiLockIfNeeded()
        }
    }
}
