package terminal.emulator.runtime

/**
 * Baseline-adaptive degradation detector for the frame-timing windows
 * reported by [FrameTimingStats].
 *
 * A fixed absolute WARN threshold cannot serve both the software-rendered
 * emulator (~555ms/frame baseline) and a real device (~17ms/frame): on the
 * emulator it never fires, on a device it misses gradual regressions. This
 * class instead learns each device's own baseline (EMA of non-degraded
 * window averages) and flags a window as degraded when its average climbs
 * to [degradationFactor]× the baseline and stays above an absolute
 * [attentionFloorNanos] — so a real regression surfaces in the logs on any
 * hardware without assuming the device type.
 *
 * Baseline update rule: only non-degraded windows move the EMA, so a
 * sustained regression keeps alerting instead of being absorbed into the
 * baseline ("boiling frog" guard).
 */
class FrameTimingTrend(
    private val degradationFactor: Double = DEFAULT_DEGRADATION_FACTOR,
    private val attentionFloorNanos: Long = DEFAULT_ATTENTION_FLOOR_NANOS,
    private val emaAlpha: Double = DEFAULT_EMA_ALPHA,
) {
    private var baselineNanos: Double? = null

    /**
     * Feeds one completed window's average render duration and returns true
     * when it is degraded relative to the learned baseline (and above the
     * attention floor). The first window only initializes the baseline.
     */
    fun observe(windowAverageNanos: Long): Boolean {
        val baseline = baselineNanos
        if (baseline == null) {
            baselineNanos = windowAverageNanos.toDouble()
            return false
        }
        val degraded = windowAverageNanos >= baseline * degradationFactor
        if (!degraded) {
            baselineNanos = baseline * (1.0 - emaAlpha) + windowAverageNanos * emaAlpha
        }
        return degraded && windowAverageNanos >= attentionFloorNanos
    }

    /** Learned baseline (ns), or null before the first window. Test/debug aid. */
    fun currentBaselineNanos(): Long? = baselineNanos?.toLong()

    companion object {
        /** 3× the learned baseline counts as a degradation worth logging. */
        const val DEFAULT_DEGRADATION_FACTOR = 3.0

        /** Below 100ms average a window is never "degraded" — sub-6 FPS
         *  is a real problem on a device and already pathological on the
         *  emulator, so the floor adds no signal loss either way. */
        const val DEFAULT_ATTENTION_FLOOR_NANOS = 100_000_000L

        /** EMA smoothing: 0.25 weights the newest window, 0.75 the history. */
        const val DEFAULT_EMA_ALPHA = 0.25
    }
}
