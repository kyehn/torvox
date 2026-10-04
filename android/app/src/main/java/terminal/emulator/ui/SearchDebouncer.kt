package terminal.emulator.ui

/** 支持取消的延时任务调度，抽象出来使防抖逻辑可在无 Looper 的 JVM 上单元测试。 */
interface DebounceScheduler {
    /** 在 [delayMillis] 之后运行 [action]，替换任何待执行的已调度动作。 */
    fun postDelayed(delayMillis: Long, action: () -> Unit)

    /** 丢弃当前已调度的动作（若有）。 */
    fun cancelPending()
}

/** 由主线程 [android.os.Handler] 支撑的 [DebounceScheduler]；生产 Compose 代码使用。JVM 单元测试不使用它（它们用假调度器）。 */
class HandlerDebounceScheduler(private val handler: android.os.Handler) : DebounceScheduler {
    private var pendingRunnable: Runnable? = null

    override fun postDelayed(delayMillis: Long, action: () -> Unit) {
        val runnable = Runnable { action() }
        pendingRunnable = runnable
        handler.postDelayed(runnable, delayMillis)
    }

    override fun cancelPending() {
        pendingRunnable?.let { handler.removeCallbacks(it) }
        pendingRunnable = null
    }
}

/**
 * 对快速连续的 [submit] 调用防抖：只有 [debounceMillis] 内最后提交的动作
 * 才会在静默期后真正运行一次。
 *
 * 纯 Kotlin（不依赖 Android）：在 JVM 上用假 [DebounceScheduler] 单元测试。
 */
class SearchDebouncer(private val debounceMillis: Long, private val scheduler: DebounceScheduler) {
    private var pendingAction: (() -> Unit)? = null

    /** 调度 [action]；先前的待执行动作被替换而非运行。 */
    fun submit(action: () -> Unit) {
        pendingAction = action
        scheduler.cancelPending()
        scheduler.postDelayed(debounceMillis) {
            // 同一性检查：动作已被后续 submit 取代的陈旧 runnable 绝不能运行新动作。
            if (pendingAction === action) {
                pendingAction = null
                action()
            }
        }
    }

    /** Drop the pending action without running it. */
    fun cancel() {
        pendingAction = null
        scheduler.cancelPending()
    }
}
