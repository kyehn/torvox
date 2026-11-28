package terminal.emulator.service

import android.app.Notification
import android.app.Service
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ServiceController
import org.robolectric.shadows.ShadowPowerManager
import terminal.emulator.R
import java.time.Duration

/**
 * Foreground-service lifecycle exercised under Robolectric: null-intent restart (process death),
 * singular/plural notification text, the user-toggled PARTIAL_WAKE_LOCK and Wi-Fi lock, and the
 * updateSessionCount stop/start decision.
 */
@RunWith(RobolectricTestRunner::class)
class TerminalForegroundServiceTest {

    private val context = ApplicationProvider.getApplicationContext<Context>()

    private fun startWith(sessionCount: Int): Service = startControllerWith(sessionCount).get()

    /**
     * 建服务并发出启动命令；需要 `destroy()` 的用例经此拿控制器。
     */
    private fun startControllerWith(sessionCount: Int): ServiceController<TerminalForegroundService> {
        val intent =
            Intent(context, TerminalForegroundService::class.java).apply {
                putExtra("session_count", sessionCount)
            }
        return Robolectric.buildService(TerminalForegroundService::class.java, intent)
            .create()
            .also { it.startCommand(0, 1) }
    }

    private fun actionIntent(action: String): Intent =
        Intent(context, TerminalForegroundService::class.java).setAction(action)

    private fun countIntent(sessionCount: Int): Intent = Intent(context, TerminalForegroundService::class.java).apply {
        putExtra("session_count", sessionCount)
    }

    @Test
    fun `null intent after process death stops instead of re-pinning`() {
        val service =
            Robolectric.buildService(TerminalForegroundService::class.java)
                .create()
                .startCommand(0, 1)
                .get()
        assertEquals(
            "restart with no sessions must be START_NOT_STICKY",
            Service.START_NOT_STICKY,
            service.onStartCommand(null, 0, 1),
        )
    }

    @Test
    fun `single session publishes the singular notification text`() {
        startWith(1)
        assertEquals(
            "singular text",
            context.getString(R.string.notification_active_single),
            postedNotificationText(),
        )
    }

    @Test
    fun `two sessions publish the counted notification text`() {
        startWith(2)
        assertEquals(
            "counted text",
            context.getString(R.string.notification_active_multiple, 2),
            postedNotificationText(),
        )
    }

    @Test
    fun `bare start without count claims nothing and holds no wake lock`() {
        ShadowPowerManager.clearWakeLocks()
        Robolectric.buildService(
            TerminalForegroundService::class.java,
            Intent(context, TerminalForegroundService::class.java),
        ).create()
            .startCommand(0, 1)
            .get()
        assertEquals(
            "zero sessions must not claim activity",
            context.getString(R.string.notification_starting),
            postedNotificationText(),
        )
        assertNull(
            "no wake lock for zero sessions",
            ShadowPowerManager.getLatestWakeLock(),
        )
    }

    @Test
    fun `explicit zero count claims nothing and holds no wake lock`() {
        ShadowPowerManager.clearWakeLocks()
        startWith(0)
        assertEquals(
            "zero sessions must not claim activity",
            context.getString(R.string.notification_starting),
            postedNotificationText(),
        )
        assertNull(
            "no wake lock for zero sessions",
            ShadowPowerManager.getLatestWakeLock(),
        )
    }

    /** DESIGN：唤醒锁不自动申请、不自动持锁——会话启动本身不得取锁。 */
    @Test
    fun `no wake lock is taken until the user asks for one`() {
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val shadowWifiManager = shadowOf(wifiManager)
        ShadowPowerManager.clearWakeLocks()
        startWith(1)
        assertNull("the wake lock must stay off by default", ShadowPowerManager.getLatestWakeLock())
        assertEquals("the wifi lock must stay off by default", 0, shadowWifiManager.getActiveLockCount())
    }

    /**
     * 通知栏按钮是唯一的申请入口：点一下持锁，再点一下释放，按钮文案跟着状态翻。
     *
     * DESIGN：锁由通知栏按钮切换，无超时机制——释放必须由用户或会话归零发起。
     */
    @Test
    fun `the wake lock action acquires the lock and offers to release it`() {
        ShadowPowerManager.clearWakeLocks()
        val controller = startControllerWith(1)
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_LOCK"),
            0,
            2,
        )
        val wakeLock = requireNotNull(ShadowPowerManager.getLatestWakeLock())
        assertTrue("the action must acquire the lock", wakeLock.isHeld)
        assertTrue(
            "the button must offer to release the lock",
            postedNotification().actions
                .map { it.title.toString() }
                .contains(context.getString(R.string.notification_wake_unlock)),
        )

        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_UNLOCK"),
            0,
            3,
        )
        assertFalse("the action must release the lock", wakeLock.isHeld)
        assertTrue(
            "the button must offer to acquire the lock again",
            postedNotification().actions
                .map { it.title.toString() }
                .contains(context.getString(R.string.notification_wake_lock)),
        )
    }

    /** 无超时机制：时间流逝不得使锁失效（旧实现在 30 分钟后自动失锁）。 */
    @Test
    fun `the wake lock is held indefinitely without a timeout`() {
        ShadowPowerManager.clearWakeLocks()
        val controller = startControllerWith(1)
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_LOCK"),
            0,
            2,
        )
        val wakeLock = requireNotNull(ShadowPowerManager.getLatestWakeLock())
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMinutes(31))
        assertTrue("no timeout may drop the lock", wakeLock.isHeld)
    }

    /**
     * Wi-Fi 锁与用户申请的唤醒锁同时在位、同时释放。
     *
     * 唤醒锁只保证 CPU 不休眠，Wi-Fi 省电仍会冻结长连接（ssh/mosh 表现为「卡住，
     * 一碰屏幕就恢复」）。
     */
    @Test
    fun `the wifi lock follows the user-requested wake lock`() {
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val shadowWifiManager = shadowOf(wifiManager)
        val controller = startControllerWith(1)
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_LOCK"),
            0,
            2,
        )
        assertEquals(
            "the wifi lock must follow the wake lock",
            1,
            shadowWifiManager.getActiveLockCount(),
        )
        controller.destroy()
        assertEquals(
            "the wifi lock must be released on destroy",
            0,
            shadowWifiManager.getActiveLockCount(),
        )
    }

    /** 没有会话就不该持有 Wi-Fi 锁：锁的代价是功耗，无人使用时不能付。 */
    @Test
    fun `no wifi lock is held without sessions`() {
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val shadowWifiManager = shadowOf(wifiManager)
        startWith(0)
        assertEquals(0, shadowWifiManager.getActiveLockCount())
    }

    @Test
    fun `updateSessionCount with zero stops the service`() {
        context.startService(Intent(context, TerminalForegroundService::class.java))
        TerminalForegroundService.updateSessionCount(context, 0)
        val stopped = shadowOf(context as android.app.Application).getNextStoppedService()
        assertNotNull("stopService must be called for a zero count", stopped)
        assertEquals(
            ComponentName(context, TerminalForegroundService::class.java),
            requireNotNull(stopped).component,
        )
    }

    /**
     * 通知上的「退出」按钮（DESIGN：常驻通知提供退出按钮）必须真的收掉全部会话并停服。
     *
     * 该动作此前**零覆盖**：按钮 action 的常量、入口分支与 `stopSelf` 都没有断言，
     * 把它接错成「只停服务不关会话」不会有任何用例变红——而那会让通知消失、
     * 会话却继续在后台跑。
     */
    @Test
    fun `exit action stops the service and reports the exit intent`() {
        val exitIntent = Intent(context, TerminalForegroundService::class.java)
            .setAction("terminal.emulator.action.EXIT")
        val service = Robolectric.buildService(TerminalForegroundService::class.java, exitIntent)
            .create()
            .get()
        assertEquals(
            "exit must not re-pin the service",
            Service.START_NOT_STICKY,
            service.onStartCommand(exitIntent, 0, 1),
        )
        assertTrue(
            "exit action must stop the service",
            shadowOf(service).isStoppedBySelf,
        )
    }

    /**
     * `updateSessionCount` 的返回值必须如实反映「是否真的发起」：后台启动被系统拒绝时
     * 调用方要靠它清掉「服务在运行」标志，否则后续 `startForegroundServiceIfNeeded`
     * 会永远跳过重启，存活会话没有常驻通知。
     */
    @Test
    fun `the notification offers a wake lock action next to exit`() {
        startWith(1)
        val actions = postedNotification().actions
        assertEquals("exit and wake lock actions expected", 2, actions.size)
        val labels = actions.map { it.title.toString() }
        assertTrue(
            "exit action missing: $labels",
            labels.contains(context.getString(R.string.notification_exit)),
        )
        assertTrue(
            "wake lock action missing: $labels",
            labels.contains(context.getString(R.string.notification_wake_lock)),
        )
    }

    @Test
    fun `a released wake lock is not taken back by the next session count update`() {
        ShadowPowerManager.clearWakeLocks()
        val controller = startControllerWith(1)
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_LOCK"),
            0,
            2,
        )
        val wakeLock = requireNotNull(ShadowPowerManager.getLatestWakeLock())
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_UNLOCK"),
            0,
            3,
        )
        assertFalse(wakeLock.isHeld)

        controller.get().onStartCommand(countIntent(2), 0, 4)
        assertFalse(
            "a session count update must not override the user's choice",
            wakeLock.isHeld,
        )
    }

    @Test
    fun `updateSessionCount reports failure when the start is refused`() {
        val refusingContext =
            object : android.content.ContextWrapper(context) {
                override fun startForegroundService(intent: Intent): ComponentName? =
                    throw android.app.ForegroundServiceStartNotAllowedException("refused")
            }
        assertFalse(
            "a refused start must not be reported as success",
            TerminalForegroundService.updateSessionCount(refusingContext, 2),
        )
    }

    /**
     * 执行锁只为存活会话而持：最后一会话关闭后锁必须释放。
     *
     * 锁的代价是功耗——PARTIAL_WAKE_LOCK 阻止 CPU 休眠，Wi-Fi 锁把射频钉在高性能
     * 档——没有人使用终端时不能继续付。
     */
    @Test
    fun `the execution locks are released when no session remains`() {
        ShadowPowerManager.clearWakeLocks()
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val shadowWifiManager = shadowOf(wifiManager)
        val controller = startControllerWith(1)
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_LOCK"),
            0,
            2,
        )
        val wakeLock = requireNotNull(ShadowPowerManager.getLatestWakeLock())
        assertTrue("the lock must be held after the user asks", wakeLock.isHeld)

        controller.get().onStartCommand(countIntent(0), 0, 3)
        assertFalse("no session means no wake lock", wakeLock.isHeld)
        assertEquals("no session means no wifi lock", 0, shadowWifiManager.getActiveLockCount())
    }

    /**
     * 通知上的「申请唤醒锁」按钮不得在无会话时把锁取回来。
     *
     * 用户的申请表达的是「有会话时替我保活」，不是「无会话时也让设备不休眠」。
     */
    @Test
    fun `the wake lock action takes no lock without sessions`() {
        ShadowPowerManager.clearWakeLocks()
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val shadowWifiManager = shadowOf(wifiManager)
        val controller = startControllerWith(0)
        controller.get().onStartCommand(
            actionIntent("terminal.emulator.action.WAKE_LOCK"),
            0,
            2,
        )
        assertNull("no session means no wake lock", ShadowPowerManager.getLatestWakeLock())
        assertEquals("no session means no wifi lock", 0, shadowWifiManager.getActiveLockCount())
        assertTrue(
            "a stale action without sessions must stop the service",
            shadowOf(controller.get()).isStoppedBySelf,
        )
    }

    private fun postedNotification(): Notification {
        val manager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager
        val notifications = shadowOf(manager).allNotifications
        assertEquals("exactly one notification expected", 1, notifications.size)
        return notifications.first()
    }

    private fun postedNotificationText(): CharSequence? =
        postedNotification().extras.getCharSequence(Notification.EXTRA_TEXT)
}
