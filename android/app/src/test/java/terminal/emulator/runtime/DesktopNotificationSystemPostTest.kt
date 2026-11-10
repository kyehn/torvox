package terminal.emulator.runtime

import android.app.Notification
import android.app.NotificationManager
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLog
import terminal.emulator.TransientMessageChannel

/**
 * 桌面通知的系统投递路径：通知被用户关掉时不得凭空消失。
 *
 * Android 13+ 的 POST_NOTIFICATIONS 未授权时 `notify()` 是**静默丢弃**，不抛异常——
 * 只看「notify 有没有抛错」会以为通知已经送达。故此处断言两条：允许时确实投递了，
 * 禁止时确实没有投递，且诊断日志在场（否则用户看到的就是「什么都没发生」）。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class DesktopNotificationSystemPostTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val manager: NotificationManager =
        checkNotNull(context.getSystemService(NotificationManager::class.java))

    private fun notification() = SessionNotification(
        sessionId = 7L,
        sessionLabel = "会话 2",
        title = "",
        body = "构建完成",
    )

    @Test
    fun `notification reaches the system when notifications are enabled`() {
        shadowOf(manager).setNotificationsEnabled(true)
        DesktopNotificationPlayer(context, TransientMessageChannel())
            .dispatch(notification(), foreground = false)
        val posted = shadowOf(manager).allNotifications
        assertEquals(1, posted.size)
        val extras = posted.first().extras
        val title = extras.getString(Notification.EXTRA_TITLE).orEmpty()
        val body = extras.getString(Notification.EXTRA_TEXT).orEmpty()
        val subText = extras.getString(Notification.EXTRA_SUB_TEXT).orEmpty()
        assertTrue("系统通知正文必须含通知正文，实际：$body", body.contains("构建完成"))
        assertTrue(
            "系统通知必须含会话序号，实际：title=$title subText=$subText",
            "$title $subText".contains("会话 2"),
        )
    }

    @Test
    fun `notification is dropped with a diagnostic when notifications are disabled`() {
        shadowOf(manager).setNotificationsEnabled(false)
        ShadowLog.clear()
        DesktopNotificationPlayer(context, TransientMessageChannel())
            .dispatch(notification(), foreground = false)
        assertEquals(0, shadowOf(manager).allNotifications.size)
        val diagnostics =
            ShadowLog.getLogsForTag("DesktopNotification").filter { it.type == android.util.Log.WARN }
        assertTrue(
            "通知被关闭时必须留下诊断，否则用户只见通知凭空消失",
            diagnostics.any { it.msg.contains("notifications disabled") },
        )
    }
}
