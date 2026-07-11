package io.torvox.ui

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import androidx.core.app.NotificationCompat
import io.torvox.R

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
    }

    private val notificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    init {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel =
                NotificationChannel(
                    CHANNEL_ID,
                    CHANNEL_NAME,
                    NotificationManager.IMPORTANCE_DEFAULT,
                )
            notificationManager.createNotificationChannel(channel)
        }
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
        notificationManager.notify(System.currentTimeMillis().toInt(), notification)
    }
}
