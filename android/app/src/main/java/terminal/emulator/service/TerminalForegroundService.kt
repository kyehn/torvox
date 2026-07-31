package terminal.emulator.service

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
import android.util.Log
import terminal.emulator.MainActivity
import terminal.emulator.R

class TerminalForegroundService : Service() {
    companion object {
        private const val CHANNEL_ID = "terminal"
        private const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_TAG = "termvox:wakelock"

        fun start(context: Context) {
            val intent = Intent(context, TerminalForegroundService::class.java)
            context.startForegroundService(intent)
        }

        fun stop(context: Context): Boolean = context.stopService(Intent(context, TerminalForegroundService::class.java))

        fun updateSessionCount(
            context: Context,
            count: Int,
        ) {
            if (count <= 0) {
                // Return value intentionally ignored: stopService(false for
                // a stopped service) is the desired end state either way.
                stop(context)
                return
            }
            val intent =
                Intent(context, TerminalForegroundService::class.java).apply {
                    putExtra("session_count", count)
                }
            try {
                context.startForegroundService(intent)
            } catch (exception: Exception) {
                // API 31+: ForegroundServiceStartNotAllowedException when
                // the app is in the background and the service is not
                // already running (e.g. system killed it and START_STICKY
                // has not restarted it yet). This must not crash the
                // render thread.
                android.util.Log.w("TerminalForegroundService", "startForegroundService failed", exception)
            }
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
        // START_STICKY restart after the process was killed: no sessions
        // survive a process death, so the service (and its PARTIAL_WAKE_LOCK)
        // has nothing to keep alive. Stop instead of re-pinning the
        // notification forever with a permanent wake lock.
        if (intent == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        sessionCount = intent.getIntExtra("session_count", 1).coerceAtLeast(1)
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
        try {
            startForeground(NOTIFICATION_ID, notification)
        } catch (exception: Exception) {
            // minSdk 33 without POST_NOTIFICATIONS permission (and some
            // vendor ROMs) makes startForeground throw SecurityException on
            // the main onStartCommand path — the static updateSessionCount
            // path already guards; this one must not crash the process.
            // KNOWN LIMITATION (round-92): the runtime's
            // foregroundServiceRunning flag was already set true by
            // startForegroundServiceIfNeeded before this call, and no
            // failure signal is sent back — a subsequent
            // startForegroundServiceIfNeeded will skip starting (stale
            // flag) until the count hits 0 via updateForegroundSessionCount
            // (round-82 reset) or stopForegroundService runs. The service
            // itself is still bound by the runtime's startService call, so
            // the wake lock and process-foreground guarantees hold; only
            // the notification is missing. Closing all sessions heals it.
            Log.e("TerminalForegroundService", "startForeground failed", exception)
        }
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
                    acquire()
                }
    }

    private fun releaseWakeLock() {
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
    }

    override fun onBind(intent: Intent?): IBinder = Binder()

    override fun onDestroy() {
        releaseWakeLock()
        super.onDestroy()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        // The service keeps running (START_STICKY) with live terminal
        // sessions. Re-acquire the wake lock instead of dropping it:
        // otherwise, with the task swiped away and the screen off, the
        // sessions' CPU and network access would be frozen with no way
        // to recover the lock (nothing calls acquireWakeLockIfNeeded
        // again after this point).
        if (wakeLock?.isHeld != true) {
            acquireWakeLockIfNeeded()
        }
    }
}
