package io.torvox.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import android.os.PowerManager
import io.torvox.MainActivity
import io.torvox.R

class TerminalForegroundService : Service() {
    companion object {
        private const val CHANNEL_ID = "torvox_terminal"
        private const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_TAG = "torvox:terminal_session"

        fun start(context: Context) {
            val intent = Intent(context, TerminalForegroundService::class.java)
            context.startForegroundService(intent)
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, TerminalForegroundService::class.java))
        }

        fun updateSessionCount(
            context: Context,
            count: Int,
        ) {
            if (count <= 0) {
                stop(context)
                return
            }
            val intent =
                Intent(context, TerminalForegroundService::class.java).apply {
                    putExtra("session_count", count)
                }
            context.startForegroundService(intent)
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null
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

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        sessionCount = intent?.getIntExtra("session_count", 1) ?: 1
        startForegroundWithSessionCount(sessionCount)
        acquireWakeLockIfNeeded()
        return START_STICKY
    }

    private fun startForegroundWithSessionCount(count: Int) {
        val text =
            if (count <= 1) {
                getString(R.string.notification_active_single)
            } else {
                getString(R.string.notification_active_plural, count)
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
        startForeground(NOTIFICATION_ID, notification)
    }

    private fun acquireWakeLockIfNeeded() {
        if (wakeLock?.isHeld == true) return
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock =
            powerManager
                .newWakeLock(
                    PowerManager.PARTIAL_WAKE_LOCK,
                    WAKE_LOCK_TAG,
                ).apply {
                    setReferenceCounted(false)
                    acquire(MAX_WAKE_LOCK_DURATION_MS)
                }
    }

    private fun releaseWakeLock() {
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
    }

    fun updateSessionCount(count: Int) {
        sessionCount = count
        if (count <= 0) {
            stopSelf()
            return
        }
        startForegroundWithSessionCount(count)
    }

    override fun onBind(intent: Intent?): IBinder = Binder()

    override fun onDestroy() {
        releaseWakeLock()
        super.onDestroy()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        releaseWakeLock()
    }
}

private const val MAX_WAKE_LOCK_DURATION_MS = 30L * 60L * 1000L
