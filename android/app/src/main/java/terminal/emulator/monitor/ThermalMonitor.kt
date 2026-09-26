package terminal.emulator.monitor

import android.content.Context
import android.os.Build
import android.os.PowerManager
import android.util.Log
import java.util.Locale
import java.util.concurrent.Executors

class ThermalMonitor(
    private val context: Context,
    private val onCritical: (() -> Unit)? = null,
) {
    private val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager

    // Written from the system thermal callback thread, read from the main
    // thread (onThermalStatusChanged) — cross-thread visibility requires
    // volatile or the dedup may log duplicate transitions.
    @Volatile
    private var lastStatus = PowerManager.THERMAL_STATUS_NONE
    private var thermalExecutor: java.util.concurrent.ExecutorService? = null
    private var thermalListener: PowerManager.OnThermalStatusChangedListener? = null

    fun register() {
        thermalListener =
            PowerManager.OnThermalStatusChangedListener { status ->
                onThermalStatusChanged(status)
            }
        try {
            val executor =
                Executors.newSingleThreadExecutor { r ->
                    Thread(r, "ThermalMonitor").apply { isDaemon = true }
                }
            thermalExecutor = executor
            pm.addThermalStatusListener(
                executor,
                thermalListener
                    ?: error("thermalListener must be initialized before use"),
            )
            Log.i(TAG, "ThermalStatusListener registered")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to register thermal status listener — not supported on this device/environment", e)
            thermalListener = null
            thermalExecutor?.shutdownNow()
            thermalExecutor = null
        }
    }

    fun unregister() {
        val listener = thermalListener
        if (listener != null) {
            pm.removeThermalStatusListener(listener)
        }
        thermalExecutor?.shutdownNow()
        thermalExecutor = null
        thermalListener = null
    }

    internal fun onThermalStatusChanged(status: Int) {
        if (status == lastStatus) return
        lastStatus = status
        val label = thermalStatusLabel(status)

        // SEVERE is a common throttling level (compile, download, charging)
        // and killing the process there loses every session without any
        // hardware risk. Only CRITICAL+ (genuine overheating) terminates.
        if (status >= PowerManager.THERMAL_STATUS_CRITICAL) {
            Log.e(TAG, "$label — killing process (CRITICAL+)")
            onCritical?.invoke()
        } else if (status >= PowerManager.THERMAL_STATUS_SEVERE) {
            Log.w(TAG, "$label — severe throttling, consider cooling")
        } else if (status >= PowerManager.THERMAL_STATUS_MODERATE) {
            Log.w(TAG, "$label — throttling may occur")
        } else {
            Log.i(TAG, "$label — returned to normal")
        }
    }
    internal fun thermalStatusLabel(status: Int): String = when (status) {
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
