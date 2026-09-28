package terminal.emulator.monitor

import android.app.ActivityManager
import android.content.ComponentCallbacks2
import android.content.Context
import android.os.Debug
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil

internal const val LOW_MEMORY_FACTOR = 2.0f

internal enum class MemoryPressure { Critical, Warning, Ok }

/** 内存压力日志等级的纯决策。系统报告内存不足时为 `Critical`，可用内存低于
 *  系统阈值两倍时为 `Warning`，否则为 `Ok`。 */
internal fun memoryPressure(availMb: Long, thresholdMb: Long, lowMemory: Boolean): MemoryPressure = when {
    lowMemory -> MemoryPressure.Critical
    availMb < thresholdMb * LOW_MEMORY_FACTOR -> MemoryPressure.Warning
    else -> MemoryPressure.Ok
}

class MemoryMonitor(private val context: Context, private val scope: CoroutineScope) {
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
            // binder/IPC 失败否则会抛出轮询循环、被 SupervisorJob 吞没，
            // 使内存监控在进程剩余生命周期内静默失效。
            LogUtil.w(TAG, "getMemoryInfo failed", exception)
            return
        }

        val pssKb: Long
        val pssStr: String?
        if (pssCounter % PSS_CHECK_INTERVAL == 0) {
            // PSS 仍经 Debug.getPss() 同步读取；不可用时已兜底。
            @Suppress("DEPRECATION")
            pssKb =
                try {
                    Debug.getPss().also { cachedPss = it }
                } catch (exception: SecurityException) {
                    LogUtil.w(TAG, "Debug.getPss() not available", exception)
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
                    LogUtil.e(
                        TAG,
                        "LOW MEMORY: avail=$availMb MB / $totalMb MB ($availPercent%), PSS=$pssStr, nativeHeap=$nativeHeapMb MB, threshold=$thresholdMb MB",
                    )
                }
            }

            MemoryPressure.Warning -> {
                lowMemoryReported = false
                LogUtil.w(
                    TAG,
                    "Memory pressure: avail=$availMb MB / $totalMb MB ($availPercent%), PSS=$pssStr, threshold=$thresholdMb MB",
                )
            }

            MemoryPressure.Ok -> {
                lowMemoryReported = false
                LogUtil.d(TAG, "Memory OK: avail=$availMb MB / $totalMb MB ($availPercent%)")
            }
        }
    }

    // 复写签名由框架固定，无法更名或迁移。
    @Suppress("DEPRECATION")
    fun onTrimMemory(level: Int) {
        when (level) {
            ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL -> {
                LogUtil.e(TAG, "TRIM_MEMORY_RUNNING_CRITICAL — reducing memory footprint")
            }

            ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW -> {
                LogUtil.w(TAG, "TRIM_MEMORY_RUNNING_LOW")
            }

            ComponentCallbacks2.TRIM_MEMORY_RUNNING_MODERATE -> {
                LogUtil.w(TAG, "TRIM_MEMORY_RUNNING_MODERATE")
            }

            ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN -> {
                LogUtil.d(TAG, "TRIM_MEMORY_UI_HIDDEN")
            }

            ComponentCallbacks2.TRIM_MEMORY_COMPLETE -> {
                // 进程已处于 LRU 底部，系统很快就会回收它
                // ——这正是应该发生的事；抢先自杀只会白白丢失所有会话而对硬件毫无益处。
                LogUtil.w(TAG, "TRIM_MEMORY_COMPLETE — process is a reclaim candidate, letting the system decide")
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
