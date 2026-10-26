package terminal.emulator.runtime

/**
 * 渲染循环的定窗帧时长统计。
 *
 * 每帧把渲染时长（`bridge.render()` 前后的 `lastRenderDone - lastRenderStart`）记入定长窗口，
 * 窗口关闭后 [takeReport] 返回 [FrameTimingReport]（均值/p95/最大）并重新开始采集
 * ——简单的非滑动窗口，使热路径每帧只有一次数组写入。
 *
 * 纯 Kotlin（不依赖 Android），故窗口与分位数计算可在 JVM 上单元测试。
 * 线程约定：渲染线程是唯一写入者；读者（日志输出）在同一线程上运行，无需同步。
 *
 * 窗口按**帧数或时长**先到者关闭：60 帧在 60 FPS 下约 1s，在 30 FPS 下约 2s；再慢的设备
 * （软件渲染模拟器实测 1.8–8 FPS）要 8–33s 才凑满 60 帧。只按帧数结算时，一段短促的
 * 交互（拖动 2.5s 只渲染出 3 帧）结束后循环转入空闲闭锁、不再产生新帧，窗口永远凑不满，
 * 这些帧的耗时**永远不会被报告**——恰恰是最需要数字的低帧率设备最缺诊断。故时长到
 * [maxWindowNanos] 即用已采集的帧出报告；快设备上永远不会触发这个上限。
 *
 * 输出另受 [DEFAULT_MIN_REPORT_INTERVAL_NANOS] 约束：两次报告的间隔不得小于它
 * （`logcat-hygiene` 规范要求 500ms）。60 帧在 122Hz 的设备上只要 492ms——攒满即结算
 * 会让输出间隔破下限。故窗口满而间隔未到时**不结算**：新帧覆盖最旧样本，报告延后到
 * 间隔满足时给出。统计保留最近的 60 帧，输出间隔恒不小于下限。
 */
class FrameTimingStats(
    private val windowSize: Int = DEFAULT_WINDOW_SIZE,
    private val maxWindowNanos: Long = DEFAULT_MAX_WINDOW_NANOS,
) {
    private val samplesNanos = LongArray(windowSize)
    private var count = 0

    /** 环形写入位置：窗口满后从最旧样本处覆盖。 */
    private var writeIndex = 0
    private var windowStartNanos = 0L
    private var lastRecordNanos = 0L

    /** 下一份报告的最早可出时刻：上次报告时刻加上间隔下限。 */
    private var reportDueNanos = 0L

    /** 是否已出过报告：首份报告不受间隔下限约束。 */
    private var hasReported = false

    /** 「窗口已满却迟迟不出报告」的告警是否已发过（每窗口只发一次）。 */
    private var overdueReported = false

    /**
     * 记录一帧的渲染时长（ns）。
     *
     * 窗口已满时新帧覆盖最旧样本：报告被输出间隔推迟期间，窗口保留最近的样本。
     */
    fun record(durationNanos: Long, atNanos: Long = System.nanoTime()) {
        if (count >= windowSize) {
            if (isReportOverdue(atNanos)) {
                overdueReported = true
                LogUtil.error(
                    "FrameTimingStats",
                    "window stayed full for more than ${maxWindowNanos / 1_000_000}ms without a report",
                )
            }
        } else {
            if (count == 0) windowStartNanos = atNanos
            count++
        }
        samplesNanos[writeIndex] = durationNanos
        writeIndex = (writeIndex + 1) % windowSize
        lastRecordNanos = atNanos
    }

    /** 自上次报告以来帧数已满、或已采集的帧跨过了时长上限时为真。 */
    fun isWindowComplete(): Boolean =
        count >= windowSize || (count > 0 && lastRecordNanos - windowStartNanos >= maxWindowNanos)

    /** 取走当前时刻的报告（生产入口）。 */
    fun takeReport(): FrameTimingReport? = takeReportAt(System.nanoTime())

    /**
     * 取走截至 [atNanos] 的报告。
     *
     * 窗口既未满帧也未超时长时返回 null；距上次报告不足
     * [DEFAULT_MIN_REPORT_INTERVAL_NANOS] 时同样返回 null，但**保留样本**——报告延后到
     * 间隔满足时给出，统计不丢。
     */
    fun takeReportAt(atNanos: Long): FrameTimingReport? {
        if (!isWindowComplete()) return null
        if (hasReported && atNanos < reportDueNanos) return null
        val frames = count
        val samples = LongArray(frames)
        System.arraycopy(samplesNanos, 0, samples, 0, frames)
        samples.sort()
        val p95Index = (frames * 95 + 99) / 100 - 1 // ceil(0.95 * n), 0-based
        count = 0
        writeIndex = 0
        hasReported = true
        reportDueNanos = atNanos + DEFAULT_MIN_REPORT_INTERVAL_NANOS
        overdueReported = false
        return FrameTimingReport(
            frameCount = frames,
            averageNanos = samples.sum() / frames,
            p95Nanos = samples[p95Index],
            maxNanos = samples[frames - 1],
        )
    }

    /**
     * 窗口已满而报告早已到期却迟迟不给：调用方不再取报告，这本身是缺陷。
     *
     * 报告在 [reportDueNanos] 到期，渲染循环每轮都调用 [takeReport]，故到期后再等满
     * [maxWindowNanos] 仍无报告即为异常。
     */
    fun isReportOverdue(atNanos: Long): Boolean =
        count >= windowSize && hasReported && !overdueReported && atNanos - reportDueNanos > maxWindowNanos

    companion object {
        const val DEFAULT_WINDOW_SIZE = 60

        /**
         * 窗口的时长上限：2s 对应 30 FPS 下的 60 帧，故 30 FPS 及更快的设备行为不变。
         */
        const val DEFAULT_MAX_WINDOW_NANOS = 2_000_000_000L

        /** 两次报告的最小间隔：`logcat-hygiene` 规范要求不小于 500ms。 */
        const val DEFAULT_MIN_REPORT_INTERVAL_NANOS = 500_000_000L
    }
}

/** 一个已完成帧时间窗口的汇总，所有时长均为纳秒。 */
data class FrameTimingReport(val frameCount: Int, val averageNanos: Long, val p95Nanos: Long, val maxNanos: Long)
