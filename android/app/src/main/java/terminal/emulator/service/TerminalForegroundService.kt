package terminal.emulator.service

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.IBinder
import android.os.PowerManager
import dagger.hilt.android.EntryPointAccessors
import terminal.emulator.MainActivity
import terminal.emulator.R
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntimeEntryPoint

class TerminalForegroundService : Service() {
    companion object {
        private const val CHANNEL_ID = "terminal"
        private const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_TAG = "terminal:wakelock"
        private const val WIFI_LOCK_TAG = "terminal:wifilock"
        private const val EXTRA_SESSION_COUNT = "session_count"
        private const val ACTION_EXIT = "terminal.emulator.action.EXIT"
        private const val ACTION_WAKE_LOCK = "terminal.emulator.action.WAKE_LOCK"
        private const val ACTION_WAKE_UNLOCK = "terminal.emulator.action.WAKE_UNLOCK"

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
         * `startForegroundServiceIfNeeded` 会永远跳过重启——存活会话没有常驻通知，
         * 后台随时可能被杀且用户看不到任何提示。
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

    private var sessionCount: Int = 0

    /**
     * 用户是否要设备保持唤醒。
     *
     * 默认 `false`：唤醒锁不自动申请、不自动持锁，锁由通知栏按钮切换
     * （DESIGN「参考 termux-app 实现唤醒锁（不自动申请），包括通知栏按钮，
     * 不自动持锁，无超时机制」）。按钮把它翻成 `true` 后，有会话时才真正持锁；
     * 用户再点一次释放，后续的会话数更新 MUST NOT 又把锁抢回来。
     */
    private var wakeLockRequested = false

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
            EntryPointAccessors
                .fromApplication(applicationContext, TerminalRuntimeEntryPoint::class.java)
                .terminalRuntime()
                .closeAllSessions()
            stopSelf()
            return START_NOT_STICKY
        }
        // 通知栏的唤醒锁按钮（DESIGN：常驻通知提供「申请唤醒锁」按钮）：与 termux
        // 上游同义——持锁时释放，没锁时申请。处理完要重发通知，按钮文案才跟着状态翻。
        if (intent?.action == ACTION_WAKE_LOCK || intent?.action == ACTION_WAKE_UNLOCK) {
            wakeLockRequested = intent.action == ACTION_WAKE_LOCK
            applyWakeLockState()
            startForegroundWithSessionCount(sessionCount)
            if (sessionCount == 0) {
                // 陈旧通知的动作可能在最后一个会话关闭、服务已停止后到达：
                // 无会话可保活，停掉服务而不是无限期挂着「启动中」。
                stopSelf()
                return START_NOT_STICKY
            }
            return START_STICKY
        }
        // 进程被杀后 START_STICKY 重启：没有会话能在进程死亡后存活，
        // 故服务已无保活对象。改为停止，而不是无休止地重新固定通知。
        if (intent == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        sessionCount =
            if (intent.hasExtra(EXTRA_SESSION_COUNT)) {
                intent.getIntExtra(EXTRA_SESSION_COUNT, 0).coerceAtLeast(0)
            } else {
                // 裸 start()（冷启动/首个会话）不带计数：不得回落成 1——此时可能尚无会话，
                // 谎报「1 个活动会话」。
                // 沿用上次已知值（冷启动为 0，即「启动中」）。
                sessionCount
            }
        // 先取锁再发通知：通知里的唤醒锁按钮文案按当前锁状态生成，顺序反了会让
        // 按钮一直显示「申请唤醒锁」，即便锁已经在手上。
        applyWakeLockState()
        startForegroundWithSessionCount(sessionCount)
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
        // 唤醒锁按钮：持锁时显示「释放唤醒锁」，没锁时显示「申请唤醒锁」（termux 上游同义）。
        val wakeLockHeld = wakeLock?.isHeld == true
        val wakeLockPending =
            PendingIntent.getService(
                this,
                if (wakeLockHeld) 3 else 2,
                Intent(this, TerminalForegroundService::class.java)
                    .setAction(if (wakeLockHeld) ACTION_WAKE_UNLOCK else ACTION_WAKE_LOCK),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        val wakeLockLabel =
            getString(
                if (wakeLockHeld) R.string.notification_wake_unlock else R.string.notification_wake_lock,
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
                .addAction(notificationAction(getString(R.string.notification_exit), exitPending))
                .addAction(notificationAction(wakeLockLabel, wakeLockPending))
                .build()
        // 缺失 POST_NOTIFICATIONS 不会使这里抛异常（平台文档：前台服务照常启动，
        // 只是通知不进抽屉、仅见于任务管理器）；实测 API 35 上拒绝该权限后
        // startForeground 仍成功（dumpsys 报 isForeground=true）。
        // 真正的失败（缺 FOREGROUND_SERVICE 权限、被判为后台启动）按 DESIGN.md 习惯节
        // 记日志并让进程崩掉，不在此静默吞掉。
        startForeground(NOTIFICATION_ID, notification)
    }

    /** 常驻通知的动作按钮：标签为空图标动作，点击即投递到本服务。 */
    private fun notificationAction(label: String, pending: PendingIntent): Notification.Action =
        Notification.Action.Builder(null, label, pending).build()

    /**
     * 获取唤醒锁（无限时持有）。
     *
     * DESIGN 要求无超时机制：只带超时的旧实现会在超时后自动失锁，而服务仍在运行、
     * `onStartCommand` 不会再被触发取回锁，灭屏期间的会话被系统冻结。锁的释放只有
     * 两个入口——用户在通知栏点「释放唤醒锁」，或会话归零——故不需要续期定时器。
     */
    @SuppressLint("WakelockTimeout") // DESIGN：无超时机制，锁由用户或会话归零显式释放。
    private fun acquireWakeLockIfNeeded() {
        if (wakeLock?.isHeld == true) return
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock =
            powerManager
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, WAKE_LOCK_TAG)
                .apply {
                    setReferenceCounted(false)
                    acquire()
                }
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

    /**
     * 服务是 `android:exported="false"` 且全仓无 `bindService` 调用，`onBind` 必须
     * 显式声明为 null——抽象成员不实现连编译都过不去，「无人调用」不是省略的理由。
     */
    override fun onBind(intent: Intent?): IBinder? = null

    private fun releaseWakeLock() {
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
    }

    private fun releaseWifiLock() {
        wifiLock?.takeIf { it.isHeld }?.release()
        wifiLock = null
    }

    /**
     * 依「用户要锁且有会话」决定持锁或放锁。
     *
     * 执行锁只为存活会话而持：无会话时用户的申请也不得把锁取回来；反之，无会话
     * （或用户已明确不要）时不得继续持有——锁的代价是功耗，没有人使用终端时不能付。
     * 通知按钮入口与常规启动入口共用这一份判据，避免两处漂移。
     */
    private fun applyWakeLockState() {
        if (sessionCount >= 1 && wakeLockRequested) {
            acquireWakeLockIfNeeded()
            acquireWifiLockIfNeeded()
        } else {
            releaseWakeLock()
            releaseWifiLock()
        }
    }

    override fun onDestroy() {
        releaseWakeLock()
        releaseWifiLock()
        super.onDestroy()
    }
}
