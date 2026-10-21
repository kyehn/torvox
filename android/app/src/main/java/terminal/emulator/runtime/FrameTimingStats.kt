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
 */
class FrameTimingStats(
    private val windowSize: Int = DEFAULT_WINDOW_SIZE,
    private val maxWindowNanos: Long = DEFAULT_MAX_WINDOW_NANOS,
) {
    private val samplesNanos = LongArray(windowSize)
    private var count = 0
    private var windowStartNanos = 0L
    private var lastRecordNanos = 0L

    /**
     * 记录一帧的渲染时长（ns）。
     *
     * 窗口已满而调用方尚未 [takeReport] 时丢弃该帧并出声：越界写会在**渲染线程**上抛
     * `ArrayIndexOutOfBoundsException`，整个终端随之消失，而丢一帧诊断数据无关紧要。
     */
    fun record(durationNanos: Long, atNanos: Long = System.nanoTime()) {
        if (count >= windowSize) {
            LogUtil.e("FrameTimingStats", "frame recorded beyond full window, dropping $durationNanos ns")
            return
        }
        if (count == 0) windowStartNanos = atNanos
        samplesNanos[count] = durationNanos
        count++
        lastRecordNanos = atNanos
    }

    /** 自上次报告以来帧数已满、或已采集的帧跨过了时长上限时为真。 */
    fun isWindowComplete(): Boolean =
        count >= windowSize || (count > 0 && lastRecordNanos - windowStartNanos >= maxWindowNanos)

    /** 返回已关闭窗口的报告并重置采集；窗口既未满帧也未超时长时返回 null。 */
    fun takeReport(): FrameTimingReport? {
        if (!isWindowComplete()) return null
        val frames = count
        val samples = LongArray(frames)
        System.arraycopy(samplesNanos, 0, samples, 0, frames)
        samples.sort()
        val p95Index = (frames * 95 + 99) / 100 - 1 // ceil(0.95 * n), 0-based
        count = 0
        return FrameTimingReport(
            frameCount = frames,
            averageNanos = samples.sum() / frames,
            p95Nanos = samples[p95Index],
            maxNanos = samples[frames - 1],
        )
    }

    companion object {
        const val DEFAULT_WINDOW_SIZE = 60

        /**
         * 窗口的时长上限：2s 对应 30 FPS 下的 60 帧，故 30 FPS 及更快的设备行为不变。
         */
        const val DEFAULT_MAX_WINDOW_NANOS = 2_000_000_000L
    }
}

/** 一个已完成帧时间窗口的汇总，所有时长均为纳秒。 */
data class FrameTimingReport(val frameCount: Int, val averageNanos: Long, val p95Nanos: Long, val maxNanos: Long)
