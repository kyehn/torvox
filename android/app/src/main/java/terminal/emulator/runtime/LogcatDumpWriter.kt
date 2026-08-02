package terminal.emulator.runtime

import android.content.Context
import android.os.Build
import android.util.Log
import java.io.BufferedWriter
import java.io.File
import java.io.FileWriter
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale
import java.util.regex.Pattern

/**
 * Persists a filtered logcat capture to app-private storage (R6: round-3
 * architecture; extracted from MainActivity).
 *
 * Owns the logcat process, the capture thread and the writer. The captured
 * lines are filtered by tag column (exact match, never substring, so
 * message bodies — possibly sensitive — are not persisted). Lifecycle:
 * [start] / [stop]; the thread restarts the logcat process on stream end
 * with a bounded retry delay.
 */
class LogcatDumpWriter(
    private val context: Context,
) {
    private val logLock = Any()
    private var logWriter: BufferedWriter? = null

    @Volatile private var logcatThread: Thread? = null

    @Volatile private var logcatProcess: Process? = null

    /** Exact tag-column match: `... T TagName: message` (single capture). */
    private val logTagPattern: Pattern =
        Pattern.compile("\\s+[A-Z]\\s+([^:]+):")

    fun start() {
        try {
            val logDir = context.getDir("logs", Context.MODE_PRIVATE)
            logDir.mkdirs()
            val timestamp =
                DateTimeFormatter.ofPattern("yyyyMMdd_HHmmss", Locale.US)
                    .format(LocalDateTime.now())
            val logFilePath = File(logDir, "term_$timestamp.log")
            logWriter = BufferedWriter(FileWriter(logFilePath, true), 8192)
            startLogcatThread()
            Log.d("LogcatDumpWriter", "File logging: ${logFilePath.absolutePath}")
        } catch (exception: Exception) {
            Log.e("LogcatDumpWriter", "Failed to init file logging", exception)
        }
    }

    fun stop() {
        try {
            synchronized(logLock) {
                // destroy() closes the process's stdin/stdout/stderr, which
                // unblocks readLine() — interrupt() alone cannot interrupt a
                // thread blocked on stream IO, so without this the old logcat
                // process leaks on every activity recreation (rotation).
                logcatProcess?.destroy()
                logcatProcess = null
                logcatThread?.interrupt()
                logcatThread = null
                logWriter?.close()
                logWriter = null
            }
        } catch (exception: Exception) {
            Log.w("LogcatDumpWriter", "stopFileLogging failed", exception)
        }
    }

    private fun startLogcatThread() {
        if (logcatThread?.isAlive == true) return
        logcatThread =
            Thread(
                {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                        Log.w(
                            "LogcatDumpWriter",
                            "Logcat capture not supported on Android 11+ — READ_LOGS permission unavailable; this path is expected to fail",
                        )
                        return@Thread
                    }
                    while (!Thread.currentThread().isInterrupted) {
                        try {
                            val process = Runtime.getRuntime().exec(arrayOf("logcat", "-v", "time", "*:D"))
                            synchronized(logLock) {
                                // stop() may have run between exec() and this
                                // assignment (activity rotation): the new
                                // logcat process must be destroyed, not leaked.
                                if (logcatThread?.isAlive != true || Thread.currentThread().isInterrupted) {
                                    process.destroy()
                                    return@Thread
                                }
                                logcatProcess = process
                            }
                            val reader = process.inputStream.bufferedReader()
                            for (line in reader.lineSequence()) {
                                val matcher = logTagPattern.matcher(line)
                                val tag = if (matcher.find()) matcher.group(1) else null
                                if (
                                    tag == "TerminalSurface" ||
                                    tag == "TerminalRuntime" ||
                                    tag == "AndroidRuntime"
                                ) {
                                    val timestamp =
                                        DateTimeFormatter.ofPattern("HH:mm:ss.SSS", Locale.US)
                                            .format(LocalDateTime.now())
                                    synchronized(logLock) {
                                        logWriter?.write("$timestamp $line\n")
                                        logWriter?.flush()
                                    }
                                }
                            }
                            Log.w("LogcatDumpWriter", "Logcat stream ended, restarting in 5s")
                            Thread.sleep(LOGCAT_RETRY_DELAY_MS)
                        } catch (e: InterruptedException) {
                            Log.w("LogcatDumpWriter", "Logcat thread interrupted, stopping")
                            Thread.currentThread().interrupt()
                            break
                        } catch (e: Exception) {
                            Log.e("LogcatDumpWriter", "Logcat capture failed, retrying in 5s: ${e.message}")
                            try {
                                Thread.sleep(LOGCAT_RETRY_DELAY_MS)
                            } catch (e: InterruptedException) {
                                Log.w("LogcatDumpWriter", "Logcat sleep interrupted, stopping")
                                Thread.currentThread().interrupt()
                                break
                            }
                        }
                    }
                },
                "FileLog",
            ).apply {
                isDaemon = true
            }
        logcatThread?.start()
    }

    private companion object {
        const val LOGCAT_RETRY_DELAY_MS = 5_000L
    }
}
