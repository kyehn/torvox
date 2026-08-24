package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FrameTimingTrendTest {
    @Test
    fun `first window only initializes the baseline`() {
        val trend = FrameTimingTrend()
        assertFalse(trend.observe(17_000_000L))
        assertEquals(17_000_000L, trend.currentBaselineNanos())
    }

    @Test
    fun `stable windows never alert and baseline converges to the steady value`() {
        val trend = FrameTimingTrend()
        // Emulator-like steady baseline: 555ms windows.
        repeat(10) { assertFalse(trend.observe(555_000_000L)) }
        val baseline = requireNotNull(trend.currentBaselineNanos())
        assertTrue(
            "baseline must stay near the steady value",
            baseline in 500_000_000L..600_000_000L,
        )
    }

    @Test
    fun `three-fold regression above the floor alerts once the baseline exists`() {
        val trend = FrameTimingTrend()
        repeat(3) { trend.observe(40_000_000L) } // device-typical 40ms baseline
        assertTrue("3x regression above the 100ms floor must alert", trend.observe(120_000_000L))
    }

    @Test
    fun `regression below the attention floor does not alert`() {
        val trend = FrameTimingTrend(attentionFloorNanos = 100_000_000L)
        repeat(3) { trend.observe(5_000_000L) }
        // 4x of a 5ms baseline but still far under the 100ms floor.
        assertFalse(trend.observe(20_000_000L))
    }

    @Test
    fun `degraded windows do not move the baseline upwards`() {
        val trend = FrameTimingTrend()
        repeat(3) { trend.observe(20_000_000L) }
        val baselineBefore = requireNotNull(trend.currentBaselineNanos())
        assertTrue(trend.observe(300_000_000L)) // sustained regression
        assertTrue(trend.observe(300_000_000L))
        val baselineAfter = requireNotNull(trend.currentBaselineNanos())
        assertEquals(
            "baseline must not absorb a sustained regression",
            baselineBefore,
            baselineAfter,
        )
    }

    @Test
    fun `recovery moves the baseline back down`() {
        val trend = FrameTimingTrend()
        repeat(3) { trend.observe(20_000_000L) }
        assertTrue(trend.observe(200_000_000L)) // one bad window
        repeat(5) { trend.observe(20_000_000L) } // recovery
        val baseline = requireNotNull(trend.currentBaselineNanos())
        assertTrue("baseline must recover below 40ms", baseline < 40_000_000L)
        assertFalse("recovered windows must not alert", trend.observe(25_000_000L))
    }

    @Test
    fun `null baseline before any observation`() {
        assertNull(FrameTimingTrend().currentBaselineNanos())
    }
}
