package io.torvox.monitor

import android.content.Context
import android.os.Build
import android.os.PowerManager
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.Executors

class ThermalMonitor(
    private val context: Context,
    private val logDir: File,
    private val onCritical: (() -> Unit)? = null,
) {
    private val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
    private var lastStatus = PowerManager.THERMAL_STATUS_NONE
    private var thermalExecutor: java.util.concurrent.ExecutorService? = null
    private var thermalListener: PowerManager.OnThermalStatusChangedListener? = null

    fun register() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return
        thermalListener =
            PowerManager.OnThermalStatusChangedListener { status ->
                onThermalStatusChanged(status)
            }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val executor =
                Executors.newSingleThreadExecutor { r ->
                    Thread(r, "ThermalMonitor").apply { isDaemon = true }
                }
            thermalExecutor = executor
            pm.addThermalStatusListener(executor, thermalListener!!)
        } else {
            @Suppress("DEPRECATION")
            pm.addThermalStatusListener(thermalListener!!)
        }
        Log.i(TAG, "ThermalStatusListener registered")
    }

    fun unregister() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val listener = thermalListener
            if (listener != null) {
                pm.removeThermalStatusListener(listener)
            }
        }
        thermalExecutor?.shutdownNow()
        thermalExecutor = null
        thermalListener = null
    }

    private fun onThermalStatusChanged(status: Int) {
        if (status == lastStatus) return
        lastStatus = status
        val label = thermalStatusLabel(status)

        if (status >= PowerManager.THERMAL_STATUS_SEVERE) {
            writeThermalLog(status, label)
            Log.e(TAG, "$label — killing process (SEVERE+)")
            onCritical?.invoke()
        } else if (status >= PowerManager.THERMAL_STATUS_MODERATE) {
            Log.w(TAG, "$label — throttling may occur")
        } else {
            Log.i(TAG, "$label — returned to normal")
        }
    }

    @Suppress("TooGenericExceptionCaught")
    private fun writeThermalLog(
        status: Int,
        label: String,
    ): File? = try {
        logDir.mkdirs()
        val timestamp = SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.US).format(Date())
        val logFile = File(logDir, "thermal_$timestamp.log")
        val content =
            buildString {
                appendLine("== Thermal Event ==")
                appendLine("Status: $label ($status)")
                appendLine("Timestamp: $timestamp")
                appendLine("API Level: ${Build.VERSION.SDK_INT}")
            }
        FileOutputStream(logFile).use { fos ->
            fos.write(content.toByteArray(Charsets.UTF_8))
            fos.fd.sync()
        }
        logFile
    } catch (e: Exception) {
        Log.e(TAG, "Failed to write thermal log", e)
        null
    }

    private fun thermalStatusLabel(status: Int): String = when (status) {
        PowerManager.THERMAL_STATUS_NONE -> "THERMAL_STATUS_NONE"
        PowerManager.THERMAL_STATUS_LIGHT -> "THERMAL_STATUS_LIGHT"
        PowerManager.THERMAL_STATUS_MODERATE -> "THERMAL_STATUS_MODERATE"
        PowerManager.THERMAL_STATUS_SEVERE -> "THERMAL_STATUS_SEVERE"
        PowerManager.THERMAL_STATUS_CRITICAL -> "THERMAL_STATUS_CRITICAL"
        PowerManager.THERMAL_STATUS_EMERGENCY -> "THERMAL_STATUS_EMERGENCY"
        PowerManager.THERMAL_STATUS_SHUTDOWN -> "THERMAL_STATUS_SHUTDOWN"
        else -> "UNKNOWN($status)"
    }

    companion object {
        private const val TAG = "ThermalMonitor"
    }
}
