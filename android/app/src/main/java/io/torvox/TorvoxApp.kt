package io.torvox

import android.app.Application
import android.util.Log
import dagger.hilt.android.HiltAndroidApp
import io.torvox.runtime.LogcatFileWriter
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@HiltAndroidApp
class TorvoxApp : Application() {
    override fun onCreate() {
        super.onCreate()
        LogcatFileWriter.init(this)
        installCrashHandler()
    }

    private fun installCrashHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                writeCrashLog(thread, throwable)
            } catch (exception: Exception) {
                Log.e("TorvoxApp", "Failed to write crash log", exception)
            }
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }

    private fun writeCrashLog(
        thread: Thread,
        throwable: Throwable,
    ) {
        val logDirectory = getDir("logs", MODE_PRIVATE)
        logDirectory.mkdirs()

        val timestamp = SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.US).format(Date())
        val crashLogFile = File(logDirectory, "crash_$timestamp.log")

        val stackTrace = StringWriter()
        throwable.printStackTrace(PrintWriter(stackTrace))

        val crashLog =
            buildString {
                appendLine("# Torvox Crash Log")
                appendLine("## Timestamp: $timestamp")
                appendLine("## Thread: ${thread.name}")
                appendLine("## Exception: ${throwable.javaClass.name}: ${throwable.message}")
                appendLine()
                appendLine("## Stack Trace:")
                appendLine(stackTrace.toString())

                val causedBy = throwable.cause
                if (causedBy != null) {
                    val causedByTrace = StringWriter()
                    causedBy.printStackTrace(PrintWriter(causedByTrace))
                    appendLine()
                    appendLine("## Caused By:")
                    appendLine(causedByTrace.toString())
                }
            }

        crashLogFile.writeText(crashLog)
        Log.e("TorvoxApp", "Crash log written to ${crashLogFile.absolutePath}")
    }
}
