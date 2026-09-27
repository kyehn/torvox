package terminal.emulator.runtime

/**
 * 渲染循环的定窗帧时长统计。
 *
 * 每帧把渲染时长（`bridge.render()` 前后的 `lastRenderDone - lastRenderStart`）记入定长窗口，
 * 窗口填满后 [takeReport] 返回 [FrameTimingReport]（均值/p95/最大）并重新开始采集
 * ——简单的非滑动窗口，使热路径每帧只有一次数组写入。
 *
 * 纯 Kotlin（不依赖 Android），故窗口与分位数计算可在 JVM 上单元测试。
 * 线程约定：渲染线程是唯一写入者；读者（日志输出）在同一线程上运行，无需同步。
 * 调用方必须在记录更多帧之前经 [takeReport] 排空每个已完成的窗口，
 * 否则下一次 [record] 会在窗口内覆写（计数以 [windowSize] 为上限）。
 *
 * 窗口大小取舍：60 帧在真机 60 FPS 下约 1s 历史，在软件渲染模拟器（~1.8 FPS 基线）下约 33s
 * ——两种情况下每窗一行汇总都是低频诊断。
 */
class FrameTimingStats(private val windowSize: Int = DEFAULT_WINDOW_SIZE) {
    private val samplesNanos = LongArray(windowSize)
    private var count = 0

    /** 记录一帧的渲染时长（ns）。调用方必须经 [takeReport] 排空每个已完成的窗口。 */
    fun record(durationNanos: Long) {
        samplesNanos[count] = durationNanos
        count++
    }

    /** 自上次报告以来已记录满 [windowSize] 帧时为真。 */
    fun isWindowComplete(): Boolean = count >= windowSize

    /** 返回已完成窗口的报告并重置采集；帧数不足 [windowSize] 时返回 null。 */
    fun takeReport(): FrameTimingReport? {
        if (count < windowSize) return null
        val samples = LongArray(windowSize)
        System.arraycopy(samplesNanos, 0, samples, 0, windowSize)
        samples.sort()
        val p95Index = (windowSize * 95 + 99) / 100 - 1 // ceil(0.95 * n), 0-based
        count = 0
        return FrameTimingReport(
            frameCount = windowSize,
            averageNanos = samples.sum() / windowSize,
            p95Nanos = samples[p95Index],
            maxNanos = samples[windowSize - 1],
        )
    }

    companion object {
        const val DEFAULT_WINDOW_SIZE = 60
    }
}

/** 一个已完成帧时间窗口的汇总，所有时长均为纳秒。 */
data class FrameTimingReport(val frameCount: Int, val averageNanos: Long, val p95Nanos: Long, val maxNanos: Long)
