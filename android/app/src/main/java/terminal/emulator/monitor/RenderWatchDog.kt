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

/**
 * 会话存活看门狗：周期判定会话是否挂在某一层，并把层级交给会话管理器处置。
 *
 * 判定本身在 [diagnoseSessionLiveness]（纯函数），看门狗只负责「周期触发判定」与
 * 「代次闸门」——陈旧看门狗在重启后触发回调会把*新*渲染线程标记为死亡。
 *
 * @param onHangDetected 挂起判定成立时的回调。第一个参数是触发本次回调的看门狗
 *   实例：处置方据此复核「本实例仍是会话的当前看门狗」，陈旧看门狗的迟到判定
 *   不得作用于已经换上新渲染线程的会话。
 */
class RenderWatchDog(
    private val diagnose: () -> SessionLivenessDiagnosis,
    private val isRunning: () -> Boolean,
    private val onHangDetected: (RenderWatchDog, SessionLivenessDiagnosis) -> Unit,
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
            // 会话不在运行时不判定：会话已切走或渲染线程已被标记死亡时，
            // 残留的忙标记只会误报。
            if (!isRunning()) continue
            // 判定输入是一次快照：起止分两次读时，可能拿到新 start 配旧 done，
            // 伪造出「帧在途」并误杀健康的渲染线程。
            val diagnosis = diagnose()
            val hungLayer = diagnosis.hungLayer ?: continue
            LogUtil.error(
                TAG,
                "session hang detected: layer=$hungLayer busy=${diagnosis.busyNanos / 1_000_000L}ms",
            )
            // 闸门只判定「本代是否仍然有效」；回调在闸门之外调用。
            //
            // 回调要取会话锁（领取关闭许可、关闭会话），而 stop() 是在持有会话锁的
            // 路径上被调用（pauseRendering、handleSessionExit、startRenderThread），
            // 它要取本闸门锁。持闸门锁回调即成 AB-BA 死锁：看门狗线程持闸门锁等
            // 会话锁，会话线程持会话锁等闸门锁——整个会话子系统冻住。
            //
            // 「回调是否仍对应当前会话」由处置方按看门狗身份复核
            // （见 TerminalRuntime.handleSessionHang）。
            val stillCurrent = synchronized(fireLock) { generation == ownGeneration }
            if (!stillCurrent) continue
            onHangDetected(this, diagnosis)
        }
    }
}
