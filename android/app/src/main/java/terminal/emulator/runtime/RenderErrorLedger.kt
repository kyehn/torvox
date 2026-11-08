package terminal.emulator.runtime

/**
 * 渲染循环的失败账本：连续错误数与四类失败诊断的窗口闸门。
 *
 * 连续错误数只管退避档位与致命上限；输出与否一律由时间窗口决定。两者必须分开：
 * 成功一帧就把连续数清零，「失败—成功—失败」的交替形态下按连续数节流每次都命中
 * 首条分支，于是逐帧记一条 error（还带完整栈），把 logcat 刷满。
 *
 * 四类诊断各持一把闸门：共用会让先到的那类吞掉后到的次数（`logcat-hygiene` 规范：
 * 慢循环与慢帧各自限频）。每类诊断的行内带窗口内次数——限频吞掉的是条数，不是次数。
 *
 * 纯 Kotlin（不依赖 Android），故窗口行为可在 JVM 上单元测试。线程约定：渲染线程
 * 是唯一写入者，与 [FrameTimingStats] 同约束。
 */
internal class RenderErrorLedger {
    /** 瞬时渲染错误（`bridge.render()` 返回负数）的窗口闸门。 */
    private val transientThrottle = CountedLogThrottle()

    /** 渲染异常（`bridge.render()` 抛异常）的窗口闸门。 */
    private val exceptionThrottle = CountedLogThrottle()

    /** 错误后恢复的窗口闸门。 */
    private val recoveryThrottle = CountedLogThrottle()

    /** 输出泵失败（`bridge.pollAll()` 抛异常）的窗口闸门。 */
    private val pollThrottle = CountedLogThrottle()

    /** 连续渲染错误数（含瞬时错误与异常）。 */
    private var consecutiveErrorCount = 0

    /** 连续渲染错误数（含本次）：退避档位与致命上限用它。 */
    val consecutiveErrors: Int
        get() = consecutiveErrorCount

    /**
     * 记一次瞬时渲染错误。
     *
     * @return 该记的一行里的窗口内次数；窗口内则返回 null（本次不记）。
     */
    fun onTransientError(atNanos: Long = System.nanoTime()): Int? {
        consecutiveErrorCount++
        return transientThrottle.record(atNanos)
    }

    /**
     * 记一次渲染异常。
     *
     * @return 该记的一行里的窗口内次数；窗口内则返回 null（本次不记）。
     */
    fun onRenderException(atNanos: Long = System.nanoTime()): Int? {
        consecutiveErrorCount++
        return exceptionThrottle.record(atNanos)
    }

    /**
     * 记一次错误后的恢复。
     *
     * @return 该记的一行里的窗口内次数；窗口内则返回 null（本次不记）。
     */
    fun onRecovery(atNanos: Long = System.nanoTime()): Int? = recoveryThrottle.record(atNanos)

    /**
     * 记一次输出泵失败。
     *
     * @return 该记的一行里的窗口内次数；窗口内则返回 null（本次不记）。
     */
    fun onPollFailure(atNanos: Long = System.nanoTime()): Int? = pollThrottle.record(atNanos)

    /** 渲染成功：连续错误数清零。 */
    fun onSuccess() {
        consecutiveErrorCount = 0
    }
}
