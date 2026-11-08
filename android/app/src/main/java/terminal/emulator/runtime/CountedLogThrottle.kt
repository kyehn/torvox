package terminal.emulator.runtime

/**
 * 带计数的限频闸门：在限频窗口内累计事件数，放行时把计数一并交出。
 *
 * 逐帧诊断若逐条输出会把 logcat 刷成洪水；若只按闸门丢弃，窗口内发生了多少次
 * 就此不可见，排障时看不出「多久一次」。本类型两者兼得：每个窗口放行一次，窗口内的
 * 事件计数随该次输出一起交出。
 *
 * 间隔以进程内单调时钟（[System.nanoTime]）判定：墙上时钟被校准时可能前跳或回跳，
 * 前跳会把两次输出的真实间隔压到下限以下，回跳会把间隔算成零而让告警长期沉默。
 *
 * 非线程安全：每个实例只由一条渲染线程使用（与 [FrameTimingStats] 同约束）。
 */
internal class CountedLogThrottle(private val intervalNanos: Long = DEFAULT_INTERVAL_NANOS) {
    /** 上次放行的时刻；[hasLogged] 为 false 时无意义。 */
    private var lastLoggedNanos = 0L

    /** 是否已放行过：首条必须可见，不得因限频而丢失。 */
    private var hasLogged = false

    /** 窗口内已累计但尚未交出的事件数。 */
    private var pendingEvents = 0

    /**
     * 记一次事件；闸门放行时返回窗口内累计的事件数，否则返回 null。
     *
     * 窗口内的计数延后交出而非丢弃：限频吞掉的是条数，不是次数。
     */
    fun record(atNanos: Long = System.nanoTime()): Int? {
        pendingEvents++
        if (hasLogged && atNanos - lastLoggedNanos < intervalNanos) {
            return null
        }
        lastLoggedNanos = atNanos
        hasLogged = true
        val count = pendingEvents
        pendingEvents = 0
        return count
    }

    /** 窗口内已累计但尚未交出的事件数（仅供测试断言）。 */
    internal val pendingCount: Int
        get() = pendingEvents

    private companion object {
        /** 诊断输出的最小间隔：与原生侧同档（500ms）。 */
        const val DEFAULT_INTERVAL_NANOS = 500_000_000L
    }
}
