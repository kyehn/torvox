package io.torvox.runtime

import android.content.Context
import android.util.Log
import java.io.File
import java.io.FileWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

object LogcatFileWriter {
    private var fileWriter: FileWriter? = null
    private var logFile: File? = null
    private val dateFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)
    private val lock = Any()

    fun init(context: Context) {
        synchronized(lock) {
            try {
                val baseDir =
                    context.getExternalFilesDir(null)
                        ?: context.getDir("logs_root", Context.MODE_PRIVATE)
                val logsDirectory =
                    File(baseDir, "logs").also { dir ->
                        if (!dir.mkdirs()) {
                            Log.w("LogcatFileWriter", "Failed to create logs directory: $dir")
                        }
                    }
                if (!logsDirectory.isDirectory) {
                    Log.e("LogcatFileWriter", "Failed to create logs directory at ${logsDirectory.absolutePath}")
                    return
                }
                val file = File(logsDirectory, "debug.log")
                logFile = file
                fileWriter = FileWriter(file, true)
                Log.d("LogcatFileWriter", "Log file: ${file.absolutePath}")
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to init file logging", exception)
            }
        }
    }

    fun write(
        tag: String,
        message: String,
    ) {
        synchronized(lock) {
            try {
                val timestamp = dateFormat.format(Date())
                fileWriter?.apply {
                    write("$timestamp $tag: $message\n")
                    flush()
                }
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to write log entry", exception)
            }
        }
    }

    fun flush() {
        synchronized(lock) {
            try {
                fileWriter?.flush()
            } catch (exception: Exception) {
                Log.e("LogcatFileWriter", "Failed to flush log file", exception)
            }
        }
    }

    fun close() {
        synchronized(lock) {
            try {
                fileWriter?.close()
                fileWriter = null
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
        }
    }
}
