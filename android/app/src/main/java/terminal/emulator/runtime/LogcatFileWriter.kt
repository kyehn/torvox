package terminal.emulator.runtime

import android.content.Context
import android.os.StrictMode
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.io.OutputStreamWriter
import java.nio.charset.StandardCharsets
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.concurrent.thread

object LogcatFileWriter {
    private var fileWriter: OutputStreamWriter? = null
    private var logFile: File? = null
    private var currentSize: Long = 0L
    private var initialized = false
    private val dateFormat = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)
    private val lock = Any()

    private const val MAX_FILE_SIZE = 1_000_000L // 1 MB
    private const val MAX_FILE_COUNT = 5
    private const val MAX_FILE_AGE_DAYS = 7L

    fun init(context: Context) {
        val prev = StrictMode.allowThreadDiskWrites()
        try {
            synchronized(lock) {
                if (initialized) {
                    // Idempotence guard: a second init (test, hot reload,
                    // activity recreation) would otherwise re-open the file
                    // writer and spawn another never-stopped LogcatFlush
                    // daemon thread per call.
                    return
                }
                try {
                    var logsDirectory = tryCreateLogsDir(context.getExternalFilesDir(null))
                    if (logsDirectory == null) {
                        logsDirectory = tryCreateLogsDir(context.getDir("logs_root", Context.MODE_PRIVATE))
                    }
                    if (logsDirectory == null) {
                        Log.e("LogcatFileWriter", "Cannot write to logs directory (both external and internal failed)")
                        return
                    }
                    purgeOldFiles(logsDirectory)
                    val file = File(logsDirectory, "debug.log")
                    currentSize = if (file.exists()) file.length() else 0L
                    logFile = file
                    fileWriter = OutputStreamWriter(FileOutputStream(file, true), StandardCharsets.UTF_8)
                    // Set the flag only after the writer is actually usable:
                    // setting it first would permanently cache an init failure
                    // (no writable directory) for the process lifetime.
                    initialized = true
                    Log.d("LogcatFileWriter", "Log file: ${file.absolutePath}")
                } catch (exception: Exception) {
                    Log.e("LogcatFileWriter", "Failed to init file logging", exception)
                }
            }
        } finally {
            StrictMode.setThreadPolicy(prev)
        }
        if (initialized) {
            thread(name = "LogcatFlush", isDaemon = true) {
                while (!Thread.currentThread().isInterrupted()) {
                    Thread.sleep(5000L)
                    timedFlush()
                }
            }
        } else {
            Log.w("LogcatFileWriter", "Log file init failed — no log file will be written")
        }
    }

    private fun tryCreateLogsDir(parent: File?): File? {
        if (parent == null) return null
        val dir = File(parent, "logs")
        if (!dir.exists() && !dir.mkdirs()) {
            Log.w("LogcatFileWriter", "Failed to create logs directory: $dir")
        }
        return if (dir.isDirectory && dir.canWrite()) dir else null
    }

    fun getLogFilePath(): String? = synchronized(lock) { logFile?.absolutePath }

    fun write(
        tag: String,
        message: String,
    ) {
        synchronized(lock) {
            try {
                maybeRotate()
                val timestamp = dateFormat.format(LocalDateTime.now())
                val line = "$timestamp $tag: $message\n"
                fileWriter?.apply {
                    write(line)
                }
                // Count UTF-8 bytes, not chars, so multi-byte text cannot
                // under-count and delay rotation past MAX_FILE_SIZE.
                currentSize += line.toByteArray(Charsets.UTF_8).size
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to write log entry", exception)
            }
        }
    }

    private fun maybeRotate() {
        if (currentSize < MAX_FILE_SIZE) return
        logFile?.let { file ->
            fileWriter?.close()
            fileWriter = null
            for (i in MAX_FILE_COUNT - 1 downTo 1) {
                val from = File(file.parentFile, "debug.$i.log")
                val to = File(file.parentFile, "debug.${i + 1}.log")
                if (from.exists()) from.renameTo(to)
            }
            val first = File(file.parentFile, "debug.1.log")
            file.renameTo(first)
            val logsDir = file.parentFile
            val newFile = File(logsDir, "debug.log")
            logFile = newFile
            fileWriter = OutputStreamWriter(FileOutputStream(newFile, false), StandardCharsets.UTF_8)
            currentSize = 0L
        }
    }

    private fun purgeOldFiles(directory: File) {
        val cutoff = System.currentTimeMillis() - MAX_FILE_AGE_DAYS * 24 * 60 * 60 * 1000L
        directory.listFiles()?.forEach { file ->
            if (file.name.startsWith("debug") && file.name.endsWith(".log") && file.lastModified() < cutoff) {
                file.delete()
            }
        }
        // Compact high indices after purging: move each file down only into an
        // empty slot, low to high. The previous loop overwrote every slot from
        // the top, collapsing all rotation files into one copy of the newest
        // content on every cold start.
        for (i in 1..MAX_FILE_COUNT) {
            val target = File(directory, "debug.$i.log")
            if (target.exists()) continue
            for (j in (i + 1)..(MAX_FILE_COUNT + 1)) {
                val source = File(directory, "debug.$j.log")
                if (source.exists()) {
                    source.renameTo(target)
                    break
                }
            }
        }
    }

    private fun timedFlush() {
        synchronized(lock) {
            try {
                fileWriter?.flush()
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to flush log file", exception)
            }
        }
    }

    fun flush() {
        timedFlush()
    }

    fun close() {
        synchronized(lock) {
            try {
                fileWriter?.close()
                fileWriter = null
                logFile = null
                currentSize = 0L
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to close log file", exception)
            }
        }
    }

    internal fun resetForTest() {
        synchronized(lock) {
            try {
                fileWriter?.close()
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to close log file during reset", exception)
            }
            fileWriter = null
            logFile = null
            currentSize = 0L
            initialized = false
        }
    }
}
