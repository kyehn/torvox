package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** `dumpsys gfxinfo` 输出的解析判据。 */
class FrameStatsTest {
    @Test
    fun parsesFrameStats() {
        val stats = requireNotNull(parseFrameStats(NONZERO_FRAME_SAMPLE))

        assertEquals(22, stats.frames)
        assertEquals(3, stats.jankyFrames)
        assertEquals(13.64, stats.jankyRatioPercent, 0.01)
        assertEquals(10L, stats.p50Millis)
        assertEquals(15L, stats.p90Millis)
        assertEquals(17L, stats.p95Millis)
        assertEquals(200L, stats.p99Millis)
    }

    @Test
    fun zeroFrameWindowIgnoresStalePercentiles() {
        val stats = requireNotNull(parseFrameStats(ZERO_FRAME_SAMPLE))

        assertEquals(FrameStats.ZERO, stats)
    }

    @Test
    fun rejectsOutputWithoutFrameCount() {
        assertNull(parseFrameStats("50th percentile: 10ms\n"))
    }
}

private const val NONZERO_FRAME_SAMPLE = """
    Total frames rendered: 22
    Janky frames: 3 (13.64%)
    50th percentile: 10ms
    90th percentile: 15ms
    95th percentile: 17ms
    99th percentile: 200ms
"""

private const val ZERO_FRAME_SAMPLE = """
    Total frames rendered: 0
    Janky frames: 0 (0.00%)
    50th percentile: 4950ms
    90th percentile: 4950ms
    95th percentile: 4950ms
    99th percentile: 4950ms
"""
