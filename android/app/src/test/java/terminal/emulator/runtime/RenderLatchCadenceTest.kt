package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * T3 backspace-latency cadence tests: the render loop must leave the 500ms idle latch (and stay on
 * the 17ms active cadence) whenever the idle clock is fresh — input writes refresh it through
 * [SessionEntry.notifyRender], including the hardware-key / IME-sendKeyEvent path wired via the
 * [terminal.emulator.bridge.Bridge.onPtyWrite] hook. Without that wake a backspace pressed after
 * >5s idle would have its shell echo wait out the full 500ms idle-latch tick.
 *
 * Mirrors the [ShouldResetScrollTest] style: the gate is a pure function plus the refresh contract
 * of the wake signal, both side-effect free and deterministic.
 */
class RenderLatchCadenceTest {

    private companion object {
        /** Synthetic staleness threshold — matches the loop's 5s idle clock at call sites. */
        const val IDLE_THRESHOLD_NANOS = 5_000_000_000L
    }

    private data class Case(val idleNanos: Long, val hasScrollMotion: Boolean, val expected: Boolean)

    private fun expectedFor(idleNanos: Long, hasScrollMotion: Boolean): Boolean =
        idleNanos > IDLE_THRESHOLD_NANOS && !hasScrollMotion

    /**
     * Full four-combination truth table of the latch gate: stale clock without scroll motion is
     * the ONLY state that selects the idle 500ms latch. A stale clock WITH scroll motion stays on
     * the active cadence (T1), and a fresh clock always stays active (input wake refreshes the
     * clock, so a backspace after idle never parks slow again).
     */
    @Test
    fun allFourCombinationsFollowIdleGate() {
        val cases =
            arrayOf(
                Case(idleNanos = 0L, hasScrollMotion = false, expected = false),
                Case(idleNanos = 0L, hasScrollMotion = true, expected = false),
                Case(idleNanos = IDLE_THRESHOLD_NANOS + 1L, hasScrollMotion = false, expected = true),
                Case(idleNanos = IDLE_THRESHOLD_NANOS + 1L, hasScrollMotion = true, expected = false),
            )
        for (case in cases) {
            assertEquals(
                "idleNanos=${case.idleNanos} hasScrollMotion=${case.hasScrollMotion}",
                case.expected,
                shouldUseIdleLatch(
                    idleNanos = case.idleNanos,
                    hasScrollMotion = case.hasScrollMotion,
                    idleThresholdNanos = IDLE_THRESHOLD_NANOS,
                ),
            )
            assertEquals(
                "truth-table oracle mismatch",
                expectedFor(case.idleNanos, case.hasScrollMotion),
                case.expected,
            )
        }
    }

    /** Boundary: exactly at the threshold the clock is not stale yet → active latch. */
    @Test
    fun clockExactlyAtThresholdStaysActive() {
        assertFalse(
            shouldUseIdleLatch(
                idleNanos = IDLE_THRESHOLD_NANOS,
                hasScrollMotion = false,
                idleThresholdNanos = IDLE_THRESHOLD_NANOS,
            ),
        )
    }

    /**
     * The wake contract the fix relies on: [SessionEntry.notifyRender] (invoked for every PTY
     * write through the onPtyWrite wiring) refreshes the idle clock and raises the render-signal,
     * so the gate immediately selects the active latch for the incoming shell echo.
     */
    @Test
    fun notifyRenderRefreshesIdleClockAndRaisesSignal() {
        val entry = SessionEntry(id = 1L, bridge = null, renderThreadRef = null, running = false)
        val staleClock = System.nanoTime() - IDLE_THRESHOLD_NANOS - 1L
        entry.lastSignalNanos = staleClock
        entry.renderSignaled.set(false)

        entry.notifyRender()

        assertTrue("input wake must raise the render signal", entry.renderSignaled.get())
        assertTrue(
            "input wake must refresh the idle clock (active latch, not 500ms idle)",
            entry.lastSignalNanos > staleClock,
        )
        val refreshedIdleNanos = System.nanoTime() - entry.lastSignalNanos
        assertFalse(
            "fresh idle clock must select the active latch",
            shouldUseIdleLatch(
                idleNanos = refreshedIdleNanos,
                hasScrollMotion = entry.hasScrollMotion(),
                idleThresholdNanos = IDLE_THRESHOLD_NANOS,
            ),
        )
    }
}
