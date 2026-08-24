package terminal.emulator.service

import android.app.Notification
import android.app.Service
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.PowerManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.shadows.ShadowPowerManager
import terminal.emulator.R

/**
 * Foreground-service lifecycle exercised under Robolectric:
 * null-intent restart (process death), singular/plural notification text,
 * the bounded PARTIAL_WAKE_LOCK acquire/release, and the
 * updateSessionCount stop/start decision.
 */
@RunWith(RobolectricTestRunner::class)
class TerminalForegroundServiceTest {

    private val context = ApplicationProvider.getApplicationContext<Context>()

    private fun startWith(sessionCount: Int): Service {
        val intent =
            Intent(context, TerminalForegroundService::class.java).apply {
                putExtra("session_count", sessionCount)
            }
        return Robolectric
            .buildService(TerminalForegroundService::class.java, intent)
            .create()
            .startCommand(0, 1)
            .get()
    }

    @Test
    fun `null intent after process death stops instead of re-pinning`() {
        val service =
            Robolectric
                .buildService(TerminalForegroundService::class.java)
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
    fun `two sessions publish the plural notification text`() {
        startWith(2)
        assertEquals(
            "plural text with count",
            context.resources.getQuantityString(R.plurals.notification_active_plural, 2, 2),
            postedNotificationText(),
        )
    }

    @Test
    fun `wake lock is acquired on start and released on destroy`() {
        val intent =
            Intent(context, TerminalForegroundService::class.java).apply {
                putExtra("session_count", 1)
            }
        val controller =
            Robolectric
                .buildService(TerminalForegroundService::class.java, intent)
                .create()
        controller.startCommand(0, 1)
        val wakeLock: PowerManager.WakeLock? = ShadowPowerManager.getLatestWakeLock()
        assertNotNull("a PARTIAL_WAKE_LOCK must be created", wakeLock)
        assertTrue("wake lock must be held while sessions run", wakeLock!!.isHeld)
        controller.destroy()
        assertFalse("wake lock must be released on destroy", wakeLock.isHeld)
    }

    @Test
    fun `updateSessionCount with zero stops the service`() {
        context.startService(Intent(context, TerminalForegroundService::class.java))
        TerminalForegroundService.updateSessionCount(context, 0)
        val stopped =
            shadowOf(context as android.app.Application)
                .getNextStoppedService()
        assertNotNull("stopService must be called for a zero count", stopped)
        assertEquals(
            ComponentName(context, TerminalForegroundService::class.java),
            stopped!!.component,
        )
    }

    private fun postedNotificationText(): CharSequence? {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager
        val notifications = shadowOf(manager).allNotifications
        assertEquals("exactly one notification expected", 1, notifications.size)
        return notifications.first().extras.getCharSequence(Notification.EXTRA_TEXT)
    }
}
