package terminal.emulator.runtime

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import terminal.emulator.service.TerminalForegroundService

/**
 * 桌面通知的系统通知 id 必须与前台服务的常驻通知分开。
 *
 * 系统通知的 id 在整个应用内共享一张表，同 id 即同一条通知。首个会话的 id 是 1，
 * 与常驻通知的 id 相同：桌面通知顶掉常驻通知，用户就丢了「退出」与「唤醒锁」入口；
 * 常驻通知刷新（会话数变化）顶掉桌面通知，用户刚收到的提醒凭空消失。
 *
 * 判据落在通知栏里的实际条数：两条通知必须并存，任一方刷新都不得吃掉另一方。
 */
@RunWith(RobolectricTestRunner::class)
class DesktopNotificationIdTest {

    private val context = ApplicationProvider.getApplicationContext<Context>()

    private fun notificationManager(): NotificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    private fun startForegroundService(sessionCount: Int) {
        val intent =
            Intent(context, TerminalForegroundService::class.java).apply {
                putExtra("session_count", sessionCount)
            }
        Robolectric.buildService(TerminalForegroundService::class.java, intent)
            .create()
            .startCommand(0, 1)
    }

    private fun dispatchDesktopNotification(sessionId: Long) {
        DesktopNotificationPlayer(context).dispatch(
            SessionNotification(
                sessionId = sessionId,
                sessionLabel = "会话 1",
                title = "",
                body = "构建完成",
            ),
            foreground = false,
        )
    }

    @Test
    fun `desktop notification does not replace the foreground service notification`() {
        startForegroundService(sessionCount = 1)
        dispatchDesktopNotification(sessionId = 1L)
        assertEquals(
            "桌面通知与常驻通知必须并存，不得互相顶替",
            2,
            shadowOf(notificationManager()).allNotifications.size,
        )
    }

    @Test
    fun `foreground service refresh does not wipe the desktop notification`() {
        startForegroundService(sessionCount = 1)
        dispatchDesktopNotification(sessionId = 1L)
        startForegroundService(sessionCount = 2)
        assertEquals(
            "常驻通知刷新不得吃掉桌面通知",
            2,
            shadowOf(notificationManager()).allNotifications.size,
        )
    }

    @Test
    fun `notification id of the same session is stable`() {
        assertEquals(
            "同一会话必须复用同一 id，后续通知顶替先前那条",
            desktopNotificationId(7L),
            desktopNotificationId(7L),
        )
    }

    @Test
    fun `notification id never collides with the foreground service notification`() {
        for (sessionId in longArrayOf(1L, 2L, 3L)) {
            assertNotEquals(
                "常驻通知占用 id 1，桌面通知不得与之相撞",
                1,
                desktopNotificationId(sessionId),
            )
        }
    }
}
