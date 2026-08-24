package terminal.emulator.ui

import android.app.Notification
import android.app.NotificationManager
import android.content.Context
import android.content.ContextWrapper
import androidx.test.core.app.ApplicationProvider
import io.mockk.every
import io.mockk.mockk
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

@RunWith(RobolectricTestRunner::class)
class TerminalNotificationHelperTest {

    private val appContext: Context
        get() = ApplicationProvider.getApplicationContext()

    private val shadowNotificationManager: org.robolectric.shadows.ShadowNotificationManager
        get() = shadowOf(appContext.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager)

    @Test
    fun `init creates the terminal notifications channel`() {
        TerminalNotificationHelper(appContext)
        val channel = shadowNotificationManager.notificationChannels.single { it.id == CHANNEL_ID }
        assertEquals("Terminal Notifications", channel.name)
        assertEquals(NotificationManager.IMPORTANCE_DEFAULT, channel.importance)
    }

    @Test
    fun `showNotification publishes title and body`() {
        TerminalNotificationHelper(appContext).showNotification("build failed", "exit code 2")

        val notification = shadowNotificationManager.allNotifications.single()
        assertEquals("build failed", notification.extras.getString(Notification.EXTRA_TITLE))
        assertEquals("exit code 2", notification.extras.getString(Notification.EXTRA_TEXT))
    }

    @Test
    fun `multiple notifications get distinct ids`() {
        // Several OSC 9 notifications can arrive within the same millisecond;
        // a time-based id would collide and silently drop the earlier one.
        val helper = TerminalNotificationHelper(appContext)
        helper.showNotification("first", "body")
        helper.showNotification("second", "body")

        assertEquals(2, shadowNotificationManager.allNotifications.size)
    }

    @Test
    fun `post notifications denied does not throw`() {
        // An exception must NOT bubble into the render thread's
        // consecutive-error counter, which would eventually misclassify
        // the session as crashed and close it (see helper's catch block).
        val throwingManager = mockk<NotificationManager>(relaxed = true)
        every { throwingManager.notify(any(), any()) } throws SecurityException("POST_NOTIFICATIONS denied")

        val denyingContext =
            object : ContextWrapper(appContext) {
                override fun getSystemService(name: String): Any? = if (name == Context.NOTIFICATION_SERVICE) throwingManager else super.getSystemService(name)
            }

        TerminalNotificationHelper(denyingContext).showNotification("t", "b")
    }

    private companion object {
        const val CHANNEL_ID = "terminal_notifications"
    }
}
