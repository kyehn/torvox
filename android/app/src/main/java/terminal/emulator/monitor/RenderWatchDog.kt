package terminal.emulator.monitor

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.runtime.LogUtil

class RenderWatchDog(
    private val getStart: () -> Long,
    private val getDone: () -> Long,
    private val isRunning: () -> Boolean,
    private val onHangDetected: () -> Unit,
    private val hangTimeoutNanos: Long = 10_000_000_000L,
    private val checkIntervalMs: Long = CHECK_INTERVAL_MS,
) {
    companion object {
        private const val CHECK_INTERVAL_MS = 2000L
        private const val TAG = "RenderWatchDog"
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var watchJob: Job? = null

    fun start() {
        if (watchJob?.isActive == true) return
        watchJob = scope.launch { watchLoop() }
    }

    fun stop() {
        val job = watchJob ?: return
        watchJob = null
        // join 以防陈旧看门狗在重启后触发 onHangDetected：
        // 该闭包读取的是*新*线程的 running 标志，会错误地把全新的渲染线程标记为死亡。
        runBlocking {
            withTimeoutOrNull(2000L) { job.cancelAndJoin() }
        }
    }

    private suspend fun watchLoop() {
        while (scope.coroutineContext.isActive) {
            delay(checkIntervalMs)
            val start = getStart()
            val done = getDone()
            val elapsed = System.nanoTime() - start
            if (start > done && elapsed > hangTimeoutNanos && isRunning()) {
                LogUtil.e(TAG, "Render hang detected: elapsed=${elapsed / 1_000_000L}ms")
                onHangDetected()
            }
        }
    }
}
