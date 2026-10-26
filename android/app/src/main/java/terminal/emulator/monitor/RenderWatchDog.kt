package terminal.emulator.monitor

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.TerminalDispatchers

/** 单帧起止时刻的不可变快照；看门狗每次只读一次，起止必定来自同一帧。 */
data class FrameMarks(val startNanos: Long = 0L, val doneNanos: Long = 0L)

class RenderWatchDog(
    private val getMarks: () -> FrameMarks,
    private val isRunning: () -> Boolean,
    private val onHangDetected: () -> Unit,
    private val hangTimeoutNanos: Long = 10_000_000_000L,
    private val checkIntervalMs: Long = CHECK_INTERVAL_MS,
) {
    companion object {
        private const val CHECK_INTERVAL_MS = 2000L
        private const val TAG = "RenderWatchDog"
    }

    private val scope = CoroutineScope(SupervisorJob() + TerminalDispatchers.background)

    // 启动与停止分属不同线程（surfaceTransitionExecutor 与渲染线程），与它处同为易变。
    @Volatile private var watchJob: Job? = null

    // 陈旧看门狗在重启后触发 onHangDetected 会把*新*渲染线程标记为死亡，故回调与 stop
    // 互斥：stop 返回后再无回调。取消协程不能替代它——取消只在挂起点生效，
    // 看门狗可能正处在 delay 之后与回调之间的窗口内。
    //
    // 用「启动代次」而非布尔位：布尔位会被下一次 start() 复位，于是同一实例重启时
    // 旧循环仍能过闸，把新渲染线程误判为死亡。代次单调递增，旧循环发现自己已过期
    // 即无权回调——不依赖调用方是否复用实例。
    private val fireLock = Any()
    private var generation = 0L

    fun start() {
        // 检查与启动必须在同一个临界区内：分开写时两个并发调用者（切会话与
        // checkSessions 的重启）都能通过 `isActive` 检查，各自跑一个 2s 轮询循环，
        // onHangDetected 被触发两次。
        synchronized(fireLock) {
            if (watchJob?.isActive == true) return
            val ownGeneration = ++generation
            watchJob = scope.launch { watchLoop(ownGeneration) }
        }
    }

    fun stop() {
        // 清空 job 与推进代次必须在**同一个临界区内**：分两次写时 `start()` 会看到
        // 「已停」而启动新一代循环，随后本次 stop 再推进一次代次，那个新循环就永远
        // 发现自己「已过期」——渲染线程再卡死也不会有人回调，等于看门狗被永久关掉。
        val job =
            synchronized(fireLock) {
                generation++
                watchJob.also { watchJob = null }
            } ?: return
        // 非阻塞：stop 在 sessionLock 内被调用（pauseRendering 的执行器线程、
        // handleSessionExit 的渲染线程），而该锁同时被主线程的
        // stopForegroundServiceIfIdle 争用——任何等待都按会话数累加为 ANR。
        job.cancel()
    }

    private suspend fun watchLoop(ownGeneration: Long) {
        // 退出条件只有一个：`delay` 在取消时抛 CancellationException。原先还套了一层
        // `scope.coroutineContext.isActive`——那是本 `WatchDog` 的 SupervisorJob，
        // 从不随 job 取消而结束，恒为真，等于一个永不生效的死条件。
        while (true) {
            delay(checkIntervalMs)
            // 一次读出整条记录：起止分两次读时，可能拿到新 start 配旧 done，
            // 伪造出 `start > done` 并误杀健康的渲染线程。
            val marks = getMarks()
            val elapsed = System.nanoTime() - marks.startNanos
            if (marks.startNanos > marks.doneNanos && elapsed > hangTimeoutNanos && isRunning()) {
                LogUtil.error(TAG, "Render hang detected: elapsed=${elapsed / 1_000_000L}ms")
                synchronized(fireLock) { if (generation == ownGeneration) onHangDetected() }
            }
        }
    }
}
