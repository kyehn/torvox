package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * P1-1 table-driven tests for [shouldResetScroll] — the pure scroll-reset
 * decision extracted from the render loop (termux `onScreenUpdated` parity).
 *
 * Covers all 16 combinations of the four input dimensions
 * (scrollActive × hasSelectionOrDrag × newOutput × recentlyScrolled), plus a
 * dedicated assertion group pinning the dual-flag protocol boundary:
 * `dirty`-only frames (selection/highlight/font-size changes → repaint
 * without PTY output) map to newOutput=false and must NEVER reset.
 */
class ShouldResetScrollTest {

    // ── Four-dimension truth table: expected = newOutput && !hasSelectionOrDrag
    //    && !scrollActive && !recentlyScrolled ────────────────────────────────

    private data class Case(
        val scrollActive: Boolean,
        val hasSelectionOrDrag: Boolean,
        val newOutput: Boolean,
        val recentlyScrolled: Boolean,
        val expected: Boolean,
    )

    private fun expectedFor(
        scrollActive: Boolean,
        hasSelectionOrDrag: Boolean,
        newOutput: Boolean,
        recentlyScrolled: Boolean,
    ): Boolean = newOutput && !hasSelectionOrDrag && !scrollActive && !recentlyScrolled

    @Test
    fun allSixteenCombinationsMatchTruthTable() {
        val cases = ArrayList<Case>(16)
        for (scrollActive in booleanArrayOf(false, true)) {
            for (hasSelectionOrDrag in booleanArrayOf(false, true)) {
                for (newOutput in booleanArrayOf(false, true)) {
                    for (recentlyScrolled in booleanArrayOf(false, true)) {
                        cases.add(
                            Case(
                                scrollActive,
                                hasSelectionOrDrag,
                                newOutput,
                                recentlyScrolled,
                                expectedFor(scrollActive, hasSelectionOrDrag, newOutput, recentlyScrolled),
                            ),
                        )
                    }
                }
            }
        }
        assertEquals(16, cases.size)
        for (case in cases) {
            val actual =
                shouldResetScroll(
                    scrollActive = case.scrollActive,
                    hasSelectionOrDrag = case.hasSelectionOrDrag,
                    newOutput = case.newOutput,
                    recentlyScrolled = case.recentlyScrolled,
                )
            assertEquals(
                "scrollActive=${case.scrollActive} hasSelectionOrDrag=${case.hasSelectionOrDrag} " +
                    "newOutput=${case.newOutput} recentlyScrolled=${case.recentlyScrolled}",
                case.expected,
                actual,
            )
        }
    }

    // ── Key semantic anchors (each dimension alone decides) ───────────────

    @Test
    fun noNewOutputNeverResets() {
        // Idle frame (no PTY output): nothing may yank the viewport.
        assertEquals(false, shouldResetScroll(scrollActive = false, hasSelectionOrDrag = false, newOutput = false))
        assertEquals(
            false,
            shouldResetScroll(scrollActive = false, hasSelectionOrDrag = false, newOutput = false, recentlyScrolled = true),
        )
    }

    @Test
    fun plainNewOutputResetsToBottom_termuxDefault() {
        // Branch A (termux parity): output arrives while browsing with no
        // selection, no SCROLL lock, no recent gesture → viewport resets.
        assertEquals(true, shouldResetScroll(scrollActive = false, hasSelectionOrDrag = false, newOutput = true))
    }

    @Test
    fun scrollLockSuppressesReset() {
        // SCROLL button toggle lock: user explicitly wants to stay browsing
        // (maps to termux's explicit isAutoScrollDisabled, verified NOT set
        // by user scrolling).
        assertEquals(
            false,
            shouldResetScroll(scrollActive = true, hasSelectionOrDrag = false, newOutput = true, recentlyScrolled = false),
        )
    }

    @Test
    fun selectionOrDragSuppressesReset() {
        // termux onScreenUpdated: do not scroll while selecting text; drag
        // maps to the same skipScrolling dimension.
        assertEquals(
            false,
            shouldResetScroll(scrollActive = false, hasSelectionOrDrag = true, newOutput = true, recentlyScrolled = false),
        )
    }

    @Test
    fun recentGestureSuppressesReset_branchB() {
        // Optional branch-B semantics (deviation from termux): a scroll
        // gesture within RECENT_SCROLL_WINDOW_NANOS suppresses one reset.
        assertEquals(
            false,
            shouldResetScroll(scrollActive = false, hasSelectionOrDrag = false, newOutput = true, recentlyScrolled = true),
        )
    }

    // ── Dual-flag protocol: dirty=true must not trigger the reset ─────────

    @Test
    fun dirtyOnlyFrameDoesNotReset() {
        // dirty≠new_output: selection/highlight/font-size changes raise the
        // native dirty flag (P2-1) which only triggers a repaint. In the
        // shouldResetScroll signature such frames are newOutput=false —
        // they must never reset regardless of the other dimensions.
        for (scrollActive in booleanArrayOf(false, true)) {
            for (hasSelectionOrDrag in booleanArrayOf(false, true)) {
                for (recentlyScrolled in booleanArrayOf(false, true)) {
                    assertEquals(
                        false,
                        shouldResetScroll(
                            scrollActive = scrollActive,
                            hasSelectionOrDrag = hasSelectionOrDrag,
                            newOutput = false,
                            recentlyScrolled = recentlyScrolled,
                        ),
                    )
                }
            }
        }
    }

    @Test
    fun cursorBlinkRepaintCountIsNotASignal_documentedRegression() {
        // Historical regression (#4): using render() count>0 as the signal
        // let cursor-blink repaints undo an upward scroll every half period.
        // The count is NOT an input to this function; only the consumed
        // new_output flag is. An idle blink frame (count>0, newOutput=false)
        // must keep the viewport where the user left it.
        assertEquals(false, shouldResetScroll(scrollActive = false, hasSelectionOrDrag = false, newOutput = false))
    }
}
