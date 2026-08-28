package terminal.emulator.runtime

import java.util.Locale

/**
 * Input→echo latency probe (emulator-performance-verification spec).
 *
 * Measures the time from a key write hitting the PTY to the first rendered
 * frame that consumed the corresponding PTY output (the echo). Pure
 * timestamp-diff logic — no clocks are read inside this class, so the
 * behavior is deterministically unit-testable; callers supply
 * `SystemClock.elapsedRealtimeNanos()` values.
 *
 * Pairing rule: each input write stamps [lastInputNanos]; when a frame
 * consumes PTY output ([onEchoFrame]), one latency sample
 * `now - lastInputNanos` is recorded IF that stamp has not been consumed
 * yet and is newer than the previously consumed stamp (sequence pairing,
 * not a time-window guess). Inputs that never produce output simply age
 * out of relevance when the next input overwrites the stamp.
 *
 * Sample capacity is a ring buffer: steady-state typing keeps the most
 * recent [capacity] samples, matching the N>=30 acceptance requirement
 * without unbounded growth.
 */
class LatencyProbe(private val capacity: Int = DEFAULT_CAPACITY) {

    init {
        require(capacity > 0) { "capacity must be positive" }
    }

    private val samples = LongArray(capacity)
    private var count = 0
    private var head = 0 // next write index in the ring

    /** Stamp of the newest input write (`elapsedRealtimeNanos`). */
    @Volatile
    var lastInputNanos: Long = 0L
        private set

    /** Stamp already paired with an echo sample; older stamps are stale. */
    @Volatile
    private var lastConsumedNanos: Long = 0L

    /** Number of latency samples recorded so far. */
    val sampleCount: Int get() = synchronized(this) { count }

    /** Called on the UI/input path right after bytes hit the PTY. */
    fun onInputWritten(elapsedRealtimeNanos: Long) {
        lastInputNanos = elapsedRealtimeNanos
    }

    /**
     * Called on the render thread when a frame consumed new PTY output.
     * Returns the latency sample in nanoseconds, or `null` when there was
     * no unconsumed input stamp newer than the last consumed one.
     */
    fun onEchoFrame(nowElapsedRealtimeNanos: Long): Long? {
        val input = lastInputNanos
        if (input == 0L || input <= lastConsumedNanos) return null
        val sample = nowElapsedRealtimeNanos - input
        // Clock anomaly (now < input): discard without recording or pairing.
        if (sample < 0) return null
        lastConsumedNanos = input
        synchronized(this) {
            samples[head] = sample
            head = (head + 1) % capacity
            if (count < capacity) count++
        }
        return sample
    }

    /** Snapshot of all recorded samples (unsorted, oldest first). */
    fun snapshot(): List<Long> = synchronized(this) {
        val out = ArrayList<Long>(count)
        val oldest = (head - count + capacity) % capacity
        for (i in 0 until count) {
            out.add(samples[(oldest + i) % capacity])
        }
        out
    }

    /**
     * Nearest-rank percentile over the recorded samples (in nanoseconds).
     * Returns `null` when fewer than [minSamples] samples are recorded —
     * the caller must surface `NOT MEASURED` instead of a number
     * (acceptance rule: no proxy metrics stand in for missing probes).
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
     * Human-readable summary for the session log / latency-report:
     * `latency n=42 p50=18.2ms p95=31.7ms`, or `latency NOT MEASURED n=3`
     * below the minimum sample count.
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
        /** Acceptance requires N>=30 samples before p50/p95 are reported. */
        const val MIN_SAMPLES: Int = 30

        const val DEFAULT_CAPACITY: Int = 512
    }
}
