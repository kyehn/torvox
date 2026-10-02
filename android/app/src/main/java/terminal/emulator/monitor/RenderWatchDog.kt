package terminal.emulator.monitor

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
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

    // 启动与停止分属不同线程（surfaceTransitionExecutor 与渲染线程），与它处同为易变。
    @Volatile private var watchJob: Job? = null

    // 陈旧看门狗在重启后触发 onHangDetected 会把*新*渲染线程标记为死亡，故回调与 stop
    // 互斥：stop 返回后再无回调。取消协程不能替代它——取消只在挂起点生效，
    // 看门狗可能正处在 delay 之后与回调之间的窗口内。
    private val fireLock = Any()
    private var stopped = false

    fun start() {
        synchronized(fireLock) { stopped = false }
        if (watchJob?.isActive == true) return
        watchJob = scope.launch { watchLoop() }
    }

    fun stop() {
        val job = watchJob ?: return
        watchJob = null
        // 非阻塞：stop 在 sessionLock 内被调用（pauseRendering 的执行器线程、
        // handleSessionExit 的渲染线程），而该锁同时被主线程的
        // stopForegroundServiceIfIdle 争用——任何等待都按会话数累加为 ANR。
        synchronized(fireLock) { stopped = true }
        job.cancel()
    }

    private suspend fun watchLoop() {
        while (scope.coroutineContext.isActive) {
            delay(checkIntervalMs)
            val start = getStart()
            val done = getDone()
            val elapsed = System.nanoTime() - start
            if (start > done && elapsed > hangTimeoutNanos && isRunning()) {
                LogUtil.e(TAG, "Render hang detected: elapsed=${elapsed / 1_000_000L}ms")
                synchronized(fireLock) { if (!stopped) onHangDetected() }
            }
        }
    }
}
