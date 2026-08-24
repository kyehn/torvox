package terminal.emulator.monitor

import android.app.ActivityManager
import android.content.ComponentCallbacks2
import android.content.Context
import android.os.Debug
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil

internal const val LOW_MEMORY_FACTOR = 2.0f

internal enum class MemoryPressure { Critical, Warning, Ok }

/** Pure decision for the memory-pressure log tier. `Critical` when the
 *  system reports low memory, `Warning` when free memory drops below twice
 *  the system threshold, otherwise `Ok`. */
internal fun memoryPressure(
    availMb: Long,
    thresholdMb: Long,
    lowMemory: Boolean,
): MemoryPressure = when {
    lowMemory -> MemoryPressure.Critical
    availMb < thresholdMb * LOW_MEMORY_FACTOR -> MemoryPressure.Warning
    else -> MemoryPressure.Ok
}

class MemoryMonitor(
    private val context: Context,
    private val scope: CoroutineScope,
) {
    private val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
    private val memInfo = ActivityManager.MemoryInfo()
    private var pollingJob: Job? = null
    private var lowMemoryReported = false
    private var pssCounter = 0
    private var cachedPss = -1L

    fun startPolling(intervalMs: Long = POLL_INTERVAL_MS) {
        stopPolling()
        pollingJob =
            scope.launch(Dispatchers.Default) {
                delay(60_000L) // defer first check past startup
                while (isActive) {
                    checkMemory()
                    delay(intervalMs)
                }
            }
    }

    fun stopPolling() {
        pollingJob?.cancel()
        pollingJob = null
    }

    fun checkMemory() {
        val availMb: Long
        val totalMb: Long
        val thresholdMb: Long
        try {
            am.getMemoryInfo(memInfo)
            availMb = memInfo.availMem / BYTES_PER_MB
            totalMb = memInfo.totalMem / BYTES_PER_MB
            thresholdMb = memInfo.threshold / BYTES_PER_MB
        } catch (exception: Exception) {
            // A binder/IPC failure would otherwise propagate out of the
            // polling loop, be swallowed by the SupervisorJob, and silently
            // disable memory monitoring for the rest of the process lifetime.
            Log.w(TAG, "getMemoryInfo failed", exception)
            return
        }

        val pssKb: Long
        val pssStr: String?
        if (pssCounter % PSS_CHECK_INTERVAL == 0) {
            @Suppress("DEPRECATION")
            pssKb =
                try {
                    Debug.getPss().also { cachedPss = it }
                } catch (e: SecurityException) {
                    Log.w(TAG, "Debug.getPss() not available", e)
                    -1L
                }
        } else {
            pssKb = cachedPss
        }
        pssCounter++
        pssStr = if (pssKb >= 0) "${pssKb}KB" else "N/A"
        val nativeHeapMb = Debug.getNativeHeapAllocatedSize() / BYTES_PER_MB

        val availPercent = if (memInfo.totalMem > 0) ((memInfo.availMem * 100) / memInfo.totalMem).toInt() else 0

        when (memoryPressure(availMb, thresholdMb, memInfo.lowMemory)) {
            MemoryPressure.Critical -> {
                if (!lowMemoryReported) {
                    lowMemoryReported = true
                    Log.e(
                        TAG,
                        "LOW MEMORY: avail=$availMb MB / $totalMb MB ($availPercent%), PSS=$pssStr, nativeHeap=$nativeHeapMb MB, threshold=$thresholdMb MB",
                    )
                }
            }

            MemoryPressure.Warning -> {
                lowMemoryReported = false
                Log.w(TAG, "Memory pressure: avail=$availMb MB / $totalMb MB ($availPercent%), PSS=$pssStr, threshold=$thresholdMb MB")
            }

            MemoryPressure.Ok -> {
                lowMemoryReported = false
                LogUtil.d(TAG, "Memory OK: avail=$availMb MB / $totalMb MB ($availPercent%)")
            }
        }
    }

    @Suppress("DEPRECATION")
    fun onTrimMemory(level: Int) {
        when (level) {
            ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL -> {
                Log.e(TAG, "TRIM_MEMORY_RUNNING_CRITICAL — reducing memory footprint")
            }

            ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW -> {
                Log.w(TAG, "TRIM_MEMORY_RUNNING_LOW")
            }

            ComponentCallbacks2.TRIM_MEMORY_RUNNING_MODERATE -> {
                Log.w(TAG, "TRIM_MEMORY_RUNNING_MODERATE")
            }

            ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN -> {
                LogUtil.d(TAG, "TRIM_MEMORY_UI_HIDDEN")
            }

            ComponentCallbacks2.TRIM_MEMORY_COMPLETE -> {
                // The process sits at the bottom of the LRU and the system
                // will reclaim it shortly — that is exactly what should
                // happen; killing ourselves first only guarantees losing
                // every session with no hardware benefit.
                Log.w(TAG, "TRIM_MEMORY_COMPLETE — process is a reclaim candidate, letting the system decide")
            }
        }
    }

    companion object {
        private const val TAG = "MemoryMonitor"
        private const val POLL_INTERVAL_MS = 30_000L
        private const val BYTES_PER_MB = 1024L * 1024L
        private const val PSS_CHECK_INTERVAL = 5
    }
}
