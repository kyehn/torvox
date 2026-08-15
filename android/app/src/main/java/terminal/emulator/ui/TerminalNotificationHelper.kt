package terminal.emulator.ui

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import androidx.core.app.NotificationCompat
import terminal.emulator.R

/**
 * Shows terminal notifications as Android system notifications when the app is
 * backgrounded, or Toast messages when the app is foregrounded.
 *
 * This matches Haven's TerminalNotifications pattern.
 */
class TerminalNotificationHelper(
    private val context: Context,
) {
    companion object {
        private const val CHANNEL_ID = "terminal_notifications"
        private const val CHANNEL_NAME = "Terminal Notifications"

        // Monotonic notification id: several OSC 9 notifications can arrive
        // within the same millisecond (render thread drains a batch); a
        // time-based id collides and the later one silently replaces the
        // earlier. This counter is process-wide and never wraps within a
        // session lifetime.
        private val nextNotificationId = java.util.concurrent.atomic.AtomicInteger(1)
    }

    private val notificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    init {
        val channel =
            NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_DEFAULT,
            )
        notificationManager.createNotificationChannel(channel)
    }

    fun showNotification(
        title: String,
        body: String,
    ) {
        val notification =
            NotificationCompat
                .Builder(context, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentTitle(title)
                .setContentText(body)
                .setAutoCancel(true)
                .build()
        try {
            notificationManager.notify(nextNotificationId.getAndIncrement(), notification)
        } catch (exception: SecurityException) {
            // POST_NOTIFICATIONS denied (Android 13+). The OSC 9 notification
            // is best-effort; a failure here must not bubble into the render
            // thread's consecutive-error counter, which would eventually
            // misclassify the session as crashed and close it.
            terminal.emulator.runtime.LogUtil.w(
                "NotificationHelper",
                "POST_NOTIFICATIONS denied, dropping OSC 9 notification",
                exception,
            )
        }
    }
}
