package io.torvox.runtime

import android.content.Context
import android.os.StrictMode
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.io.OutputStreamWriter
import java.nio.charset.StandardCharsets
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.concurrent.thread

object LogcatFileWriter {
    private var fileWriter: OutputStreamWriter? = null
    private var logFile: File? = null
    private var currentSize: Long = 0L
    private var initialized = false
    private val dateFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)
    private val lock = Any()

    private const val MAX_FILE_SIZE = 1_000_000L // 1 MB
    private const val MAX_FILE_COUNT = 5
    private const val MAX_FILE_AGE_DAYS = 7L

    fun init(context: Context) {
        val prev = StrictMode.allowThreadDiskWrites()
        try {
            synchronized(lock) {
                try {
                    initialized = true
                    val baseDir =
                        context.getExternalFilesDir(null)
                            ?: context.getDir("logs_root", Context.MODE_PRIVATE)
                    val logsDirectory =
                        File(baseDir, "logs").also { dir ->
                            if (!dir.mkdirs()) {
                                Log.w("LogcatFileWriter", "Failed to create logs directory: $dir")
                            }
                        }
                    if (!logsDirectory.isDirectory || !logsDirectory.canWrite()) {
                        Log.e("LogcatFileWriter", "Cannot write to logs directory at ${logsDirectory.absolutePath}")
                        return
                    }
                    purgeOldFiles(logsDirectory)
                    val file = File(logsDirectory, "debug.log")
                    currentSize = if (file.exists()) file.length() else 0L
                    logFile = file
                    fileWriter = OutputStreamWriter(FileOutputStream(file, true), StandardCharsets.UTF_8)
                    Log.d("LogcatFileWriter", "Log file: ${file.absolutePath}")
                } catch (exception: Exception) {
                    Log.e("LogcatFileWriter", "Failed to init file logging", exception)
                }
            }
        } finally {
            StrictMode.setThreadPolicy(prev)
        }
        thread(name = "LogcatFlush", isDaemon = true) {
            while (!Thread.currentThread().isInterrupted()) {
                Thread.sleep(5000L)
                timedFlush()
            }
        }
    }

    fun getLogFilePath(): String? = synchronized(lock) { logFile?.absolutePath }

    fun write(
        tag: String,
        message: String,
    ) {
        synchronized(lock) {
            try {
                maybeRotate()
                val timestamp = dateFormat.format(Date())
                fileWriter?.apply {
                    write("$timestamp $tag: $message\n")
                }
                currentSize += timestamp.length + tag.length + message.length + 4
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
        // Compact high indices after purging
        for (i in MAX_FILE_COUNT downTo 1) {
            val file = File(directory, "debug.${i + 1}.log")
            if (file.exists()) {
                val target = File(directory, "debug.$i.log")
                file.renameTo(target)
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
