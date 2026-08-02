package terminal.emulator.monitor

import android.os.Handler
import android.os.Looper
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

@Suppress("DEPRECATION")
class AnrWatchDog(
    private val logDir: File,
    private val timeoutMs: Long = ANR_TIMEOUT_MILLIS,
) {
    private val mainHandler = Handler(Looper.getMainLooper())

    private val running = AtomicBoolean(false)
    private var watchThread: Thread? = null
    private val anrInProgress = AtomicBoolean(false)
    private val completed = AtomicBoolean(false)

    // Bumped on every start(); a watchdog thread whose generation no longer
    // matches exits — so a stop()→start() cycle where the old thread
    // survived the join timeout cannot leave two watchers alive (round-115).
    private val generation = AtomicInteger(0)

    fun start() {
        // CAS: two concurrent callers must not start two watchdog threads
        // (each may kill the process) (round-114).
        if (!running.compareAndSet(false, true)) return
        val myGeneration = generation.incrementAndGet()
        completed.set(false)
        anrInProgress.set(false)
        watchThread =
            Thread({ watchLoop(myGeneration) }, "AnrWatchDog").apply {
                isDaemon = true
                start()
            }
    }

    // Defensive API: no production caller today (the watchdog lives for the
    // whole process). Keep the generation handshake so a future caller
    // cannot leak a stale watcher (round-115).
    fun stop() {
        running.set(false)
        generation.incrementAndGet()
        watchThread?.apply {
            interrupt()
            join(1000)
        }
        watchThread = null
    }

    private fun watchLoop(myGeneration: Int) {
        // Warm-up window: cold start (Hilt injection, first Compose frame,
        // DataStore reads) routinely exceeds 5s on slow devices; a single
        // false positive kills the process and loses every session. Skip
        // checks until the app has been running for a while.
        val startUpNanos = System.nanoTime()
        while (System.nanoTime() - startUpNanos < WARM_UP_MILLIS * 1_000_000L) {
            if (!running.get() || generation.get() != myGeneration) return
            try {
                Thread.sleep(WARM_UP_SLEEP_MILLIS)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
                return
            }
        }
        while (running.get() && generation.get() == myGeneration) {
            if (anrInProgress.get()) {
                try {
                    Thread.sleep(timeoutMs)
                } catch (_: InterruptedException) {
                    Thread.currentThread().interrupt()
                    break
                }
                continue
            }
            completed.set(false)
            mainHandler.post {
                completed.set(true)
            }
            val startMs = System.currentTimeMillis()
            try {
                while (running.get() && generation.get() == myGeneration) {
                    val elapsed = System.currentTimeMillis() - startMs
                    if (elapsed >= timeoutMs) {
                        onAnrDetected()
                        break
                    }
                    if (completed.get()) break
                    Thread.sleep(BUSY_WAIT_SLEEP_MILLIS)
                }
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
                break
            }
        }
    }

    @Suppress("TooGenericExceptionCaught")
    private fun onAnrDetected() {
        if (!anrInProgress.compareAndSet(false, true)) return
        try {
            val suppressed = !BootGuard.autoKillEnabled
            if (suppressed) {
                // BootGuard has already suppressed killing after repeated
                // exits. Writing a full thread dump every 5 s while the
                // main thread stays blocked would fill the data partition
                // (dumps are fsync'd and never rotated), so only log to
                // logcat in this state.
                Log.e("AnrWatchDog", "ANR suppressed by BootGuard; skipping dump")
                return
            }
            val stackTraces = StringBuilder()
            val mainStackTrace = Looper.getMainLooper().thread.stackTrace
            stackTraces.appendLine("== ANR Detected ==")
            stackTraces.appendLine("Timeout: ${timeoutMs}ms")
            stackTraces.appendLine()
            stackTraces.appendLine("--- Main Thread ---")
            for (element in mainStackTrace) {
                stackTraces.appendLine("\tat $element")
            }
            stackTraces.appendLine()
            stackTraces.appendLine("--- All Threads ---")
            val threadStacks = Thread.getAllStackTraces()
            for ((thread, trace) in threadStacks) {
                if (thread == Looper.getMainLooper().thread) continue
                stackTraces.appendLine("${thread.name} (priority=${thread.priority}, state=${thread.state})")
                for (element in trace) {
                    stackTraces.appendLine("\tat $element")
                }
                stackTraces.appendLine()
            }

            val timestamp =
                DateTimeFormatter.ofPattern("yyyy-MM-dd_HH-mm-ss", Locale.US).format(LocalDateTime.now())
            val logFile = File(logDir, "anr_$timestamp.log")
            logDir.mkdirs()

            val bytes = stackTraces.toString().toByteArray(Charsets.UTF_8)
            FileOutputStream(logFile).use { fos ->
                fos.write(bytes)
                try {
                    fos.fd.sync()
                } catch (e: Exception) {
                    Log.w("AnrWatchDog", "fsync failed for ANR log", e)
                }
            }

            Log.e("AnrWatchDog", "ANR written to ${logFile.absolutePath}")

            Log.e("AnrWatchDog", "Killing process due to ANR")
            BootGuard.exit(logDir, "ANR")
        } catch (e: Exception) {
            Log.e("AnrWatchDog", "Unhandled exception in ANR handler", e)
        } finally {
            anrInProgress.set(false)
        }
    }

    companion object {
        private const val ANR_TIMEOUT_MILLIS = 5_000L
        private const val BUSY_WAIT_SLEEP_MILLIS = 100L
        private const val WARM_UP_MILLIS = 20_000L
        private const val WARM_UP_SLEEP_MILLIS = 500L
    }
}
