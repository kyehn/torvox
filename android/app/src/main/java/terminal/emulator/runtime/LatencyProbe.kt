package terminal.emulator.runtime

import java.util.Locale

/**
 * 输入→回显延迟探针。测量按键写入抵达 PTY 到首帧消费掉对应 PTY 输出（回显）之间的时间。
 *
 * 纯时间戳差逻辑——本类内不读取任何时钟，故行为可确定性地单元测试；
 * 时钟值由调用方以 `SystemClock.elapsedRealtimeNanos()` 提供。
 *
 * 配对规则：每次输入写入给 [lastInputNanos] 盖戳；当某帧消费 PTY 输出（[onEchoFrame]）时，
 * 若该戳尚未被消费且新于上一个已消费戳，则记录一个 `now - lastInputNanos` 样本
 * （按序列配对，而非时间窗口猜测）。永不产生输出的输入会在下次输入覆写该戳时自然失效。
 *
 * 样本容量是环形缓冲：稳态打字只保留最近 [capacity] 个样本，
 * 满足 N>=30 的验收要求而无无界增长。
 */
class LatencyProbe(private val capacity: Int = DEFAULT_CAPACITY) {

    init {
        require(capacity > 0) { "capacity must be positive" }
    }

    private val samples = LongArray(capacity)
    private var count = 0
    private var head = 0 // 环形缓冲中下一个写入位置

    /** 最新一次输入写入的时间戳（`elapsedRealtimeNanos`）。 */
    @Volatile
    var lastInputNanos: Long = 0L
        private set

    /** 已与回显样本配对的戳；更早的戳已陈旧。 */
    @Volatile
    private var lastConsumedNanos: Long = 0L

    /** 迄今记录的延迟样本数。 */
    val sampleCount: Int get() = synchronized(this) { count }

    /** 在 UI/输入路径上，字节抵达 PTY 后立即调用。 */
    fun onInputWritten(elapsedRealtimeNanos: Long) {
        lastInputNanos = elapsedRealtimeNanos
    }

    /**
     * 在渲染线程上，某帧消费了新的 PTY 输出时调用。
     * 返回纳秒为单位的延迟样本；没有比上次已消费戳更新的未消费输入戳时返回 `null`。
     */
    fun onEchoFrame(nowElapsedRealtimeNanos: Long): Long? {
        val input = lastInputNanos
        if (input == 0L || input <= lastConsumedNanos) return null
        val sample = nowElapsedRealtimeNanos - input
        // 时钟异常（now < input）：不记录也不配对，直接丢弃。
        if (sample < 0) return null
        lastConsumedNanos = input
        synchronized(this) {
            samples[head] = sample
            head = (head + 1) % capacity
            if (count < capacity) count++
        }
        return sample
    }

    /** 所有已记录样本的快照（未排序，最旧在前）。 */
    fun snapshot(): List<Long> = synchronized(this) {
        val out = ArrayList<Long>(count)
        val oldest = (head - count + capacity) % capacity
        for (sampleIndex in 0 until count) {
            out.add(samples[(oldest + sampleIndex) % capacity])
        }
        out
    }

    /**
     * 已记录样本上的最近秩分位数（纳秒）。
     * 样本数少于 [minSamples] 时返回 `null`——调用方必须呈现
     * `NOT MEASURED` 而非数字（验收规则：不得用代理指标代替缺失的探针）。
     */
    fun percentile(percentile: Double, minSamples: Int = MIN_SAMPLES): Long? {
        require(percentile in 0.0..100.0) { "percentile must be between 0.0 and 100.0, got $percentile" }
        val all = snapshot()
        if (all.size < minSamples) return null
        val sorted = all.sorted()
        val rank = kotlin.math.ceil(percentile / 100.0 * sorted.size)
            .toInt()
            .coerceIn(1, sorted.size)
        return sorted[rank - 1]
    }

    /**
     * 供会话日志/延迟报告使用的可读汇总：
     * `latency n=42 p50=18.2ms p95=31.7ms`；样本数低于下限时为 `latency NOT MEASURED n=3`。
     */
    fun report(minSamples: Int = MIN_SAMPLES): String {
        val p50 = percentile(50.0, minSamples)
        val p95 = percentile(95.0, minSamples)
        return if (p50 == null || p95 == null) {
            "latency NOT MEASURED n=$sampleCount"
        } else {
            String.format(
                Locale.US,
                "latency n=%d p50=%.1fms p95=%.1fms",
                sampleCount,
                p50 / 1_000_000.0,
                p95 / 1_000_000.0,
            )
        }
    }

    companion object {
        /** 验收要求 p50/p95 上报前必须有 N>=30 个样本。 */
        const val MIN_SAMPLES: Int = 30

        const val DEFAULT_CAPACITY: Int = 512
    }
}
