package terminal.emulator.runtime

/**
 * Fixed-window frame-duration statistics for the render loop.
 *
 * Every frame records the render duration (`lastRenderDone - lastRenderStart`
 * around `bridge.render()`) into a fixed-size window. Once the window is
 * full, [takeReport] returns a [FrameTimingReport] (average / p95 / max) and
 * collection restarts — a simple, non-sliding window that keeps the hot path
 * to exactly one array store per frame.
 *
 * Pure Kotlin (no Android dependency) so the window and percentile math is
 * unit-testable on the JVM. Threading contract: the render thread is the only
 * writer; readers (log output) run on the same thread, so no synchronization
 * is needed. The caller must drain each completed window via [takeReport]
 * before recording more frames, or the next [record] would overwrite within
 * the window (count is capped at [windowSize]).
 *
 * Window size trade-off: 60 frames is ~1s of history at 60 FPS on a real
 * device, and ~33s on the software-rendered emulator (~1.8 FPS baseline) —
 * either way one summary line per window is a low-frequency diagnostic.
 */
class FrameTimingStats(
    private val windowSize: Int = DEFAULT_WINDOW_SIZE,
) {
    private val samplesNanos = LongArray(windowSize)
    private var count = 0

    /** Records one frame's render duration (ns). The caller must drain each completed window via [takeReport]. */
    fun record(durationNanos: Long) {
        samplesNanos[count] = durationNanos
        count++
    }

    /** True once [windowSize] frames have been recorded since the last report. */
    fun isWindowComplete(): Boolean = count >= windowSize

    /**
     * Returns the report for the completed window and resets collection, or
     * null while fewer than [windowSize] frames have been recorded.
     */
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

/**
 * Summary of one completed frame-timing window. All durations in nanoseconds.
 */
data class FrameTimingReport(
    val frameCount: Int,
    val averageNanos: Long,
    val p95Nanos: Long,
    val maxNanos: Long,
)
