package io.torvox

import android.app.Application
import android.util.Log
import dagger.hilt.android.HiltAndroidApp
import io.torvox.bridge.TorvoxBridge
import io.torvox.monitor.AnrWatchDog
import io.torvox.monitor.BootGuard
import io.torvox.monitor.MemoryMonitor
import io.torvox.monitor.SelfExit
import io.torvox.monitor.StrictModeConfig
import io.torvox.monitor.ThermalMonitor
import io.torvox.runtime.LogcatFileWriter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File
import java.io.FileOutputStream
import java.io.PrintWriter
import java.io.StringWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@HiltAndroidApp
class TorvoxApp : Application() {
    private var anrWatchDog: AnrWatchDog? = null
    private var memoryMonitor: MemoryMonitor? = null
    private var thermalMonitor: ThermalMonitor? = null
    private val monitorScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        val logDir = getDir("logs", MODE_PRIVATE)
        BootGuard(logDir).rotateLogs()
        BootGuard(logDir).check()
        StrictModeConfig.install()
        LogcatFileWriter.init(this)
        TorvoxBridge.initLogger()
        LogcatFileWriter.getLogFilePath()?.let { TorvoxBridge.setLogFilePath(it) }
        installAnrWatchDog()
        installMemoryMonitor()
        installThermalMonitor()
        installCrashHandler()
        monitorScope.launch {
            delay(HEALTHY_UPTIME_MS)
            BootGuard(logDir).markHealthy()
        }
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        memoryMonitor?.onTrimMemory(level)
    }

    private fun installAnrWatchDog() {
        val logDir = getDir("logs", MODE_PRIVATE)
        anrWatchDog = AnrWatchDog(logDir, ANR_TIMEOUT_MILLIS).also { it.start() }
    }

    private fun installMemoryMonitor() {
        val logDir = getDir("logs", MODE_PRIVATE)
        memoryMonitor =
            MemoryMonitor(this, monitorScope) {
                SelfExit.exit(logDir, "Critical memory pressure")
            }.also {
                it.startPolling()
            }
    }

    private fun installThermalMonitor() {
        val logDir = getDir("logs", MODE_PRIVATE)
        thermalMonitor =
            ThermalMonitor(this, logDir) {
                SelfExit.exit(logDir, "Thermal SEVERE+")
            }.also { it.register() }
    }

    private fun installCrashHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                writeCrashLog(thread, throwable)
                BootGuard(getDir("logs", MODE_PRIVATE)).recordExit()
            } catch (exception: Exception) {
                Log.e("TorvoxApp", "Failed to write crash log", exception)
            }
            SelfExit.exit(getDir("logs", MODE_PRIVATE), "Uncaught exception on ${thread.name}")
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

        FileOutputStream(crashLogFile).use { fos ->
            fos.write(crashLog.toByteArray(Charsets.UTF_8))
        }
        Log.e("TorvoxApp", "Crash log written to ${crashLogFile.absolutePath}")
    }

    companion object {
        private const val ANR_TIMEOUT_MILLIS = 5_000L
        private const val HEALTHY_UPTIME_MS = 10 * 60 * 1000L
    }
}
