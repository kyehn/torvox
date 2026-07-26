package io.term.monitor

import android.os.Handler
import android.os.Looper
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean

@Suppress("DEPRECATION")
class AnrWatchDog(
    private val logDir: File,
    private val timeoutMs: Long = ANR_TIMEOUT_MILLIS,
) {
    private val mainHandler = Handler(Looper.getMainLooper())

    @Volatile private var running = false
    private var watchThread: Thread? = null
    private val anrInProgress = AtomicBoolean(false)
    private val completed = AtomicBoolean(false)

    fun start() {
        if (running) return
        running = true
        completed.set(false)
        anrInProgress.set(false)
        watchThread =
            Thread({ watchLoop() }, "AnrWatchDog").apply {
                isDaemon = true
                start()
            }
    }

    fun stop() {
        running = false
        watchThread?.apply {
            interrupt()
            join(1000)
        }
        watchThread = null
    }

    private fun watchLoop() {
        while (running) {
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
                while (running) {
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
                SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.US).format(Date())
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
            SelfExit.exit(logDir, "ANR")
        } catch (e: Exception) {
            Log.e("AnrWatchDog", "Unhandled exception in ANR handler", e)
        } finally {
            anrInProgress.set(false)
        }
    }

    companion object {
        private const val ANR_TIMEOUT_MILLIS = 5_000L
        private const val BUSY_WAIT_SLEEP_MILLIS = 100L
    }
}
