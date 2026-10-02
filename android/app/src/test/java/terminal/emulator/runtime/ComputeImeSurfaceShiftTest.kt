package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure IME surface-shift computation behind TerminalScreen's terminal-Surface offset:
 * only the content the keyboard would hide *and* that does not fit above the modifier bar
 * moves up, so sparse sessions stay pixel-identical while grid-filling sessions shift by the
 * full keyboard height.
 *
 * Geometry mirrors the measured Android 15 x86_64 emulator (1080x2400, density 420):
 * container 2274px, modifier bar 189px (72dp), keyboard 820px after the navigation bar
 * (883px) is deducted, cell height 45px.
 */
class ComputeImeSurfaceShiftTest {

    private val containerHeightPx = 2274
    private val modifierBarHeightPx = 189
    private val keyboardPx = 820
    private val cellHeightPx = 45
    private val gridHeightPx = containerHeightPx - modifierBarHeightPx
    private val visibleGridPx = gridHeightPx - keyboardPx

    private fun shiftFor(contentBottomPx: Int, imeBottomPx: Int = keyboardPx) = computeImeSurfaceShift(
        contentBottomPx = contentBottomPx,
        surfaceHeightPx = containerHeightPx,
        modifierBarHeightPx = modifierBarHeightPx,
        imeBottomPx = imeBottomPx,
    )

    // ── sparse content stays put ────────────────────────────────────────────

    /** Shell prompt on grid row 0: it fits above the bar, so nothing may move. */
    @Test
    fun `single row prompt does not move`() {
        assertEquals(0, shiftFor(contentBottomPx = cellHeightPx))
    }

    @Test
    fun `empty viewport does not move`() {
        assertEquals(0, shiftFor(contentBottomPx = 0))
    }

    @Test
    fun `content exactly filling the visible area does not move`() {
        assertEquals(0, shiftFor(contentBottomPx = visibleGridPx))
    }

    @Test
    fun `one row below the visible area moves exactly one row`() {
        assertEquals(cellHeightPx, shiftFor(contentBottomPx = visibleGridPx + cellHeightPx))
    }

    // ── grid-filling content moves by the full keyboard height ───────────────

    @Test
    fun `content filling the grid moves by the keyboard height`() {
        assertEquals(keyboardPx, shiftFor(contentBottomPx = gridHeightPx))
    }

    /** Shift never exceeds the keyboard height: that would uncover empty background. */
    @Test
    fun `shift never exceeds the keyboard height`() {
        assertEquals(keyboardPx, shiftFor(contentBottomPx = gridHeightPx * 4))
    }

    // ── keyboard hidden: the terminal never moves ────────────────────────────

    @Test
    fun `keyboard hidden never moves the terminal`() {
        assertEquals(0, shiftFor(contentBottomPx = cellHeightPx, imeBottomPx = 0))
        assertEquals(0, shiftFor(contentBottomPx = gridHeightPx, imeBottomPx = 0))
    }

    // ── monotonic while typing with the keyboard open ───────────────────────

    @Test
    fun `shift grows monotonically as content grows`() {
        var previous = 0
        for (contentBottomPx in 0..(gridHeightPx + cellHeightPx) step cellHeightPx) {
            val shift = shiftFor(contentBottomPx)
            assertEquals("位移必须随内容下沿单调不减", true, shift >= previous)
            previous = shift
        }
        assertEquals(keyboardPx, previous)
    }

    // ── degenerate geometry ────────────────────────────────────────────────

    /** Keyboard plus bar taller than the container: the bar's top leaves the window,
     * so the shift degenerates to pushing the content bottom to that off-screen line. */
    @Test
    fun `keyboard covering the whole container degenerates to content bottom`() {
        assertEquals(
            cellHeightPx + modifierBarHeightPx,
            shiftFor(contentBottomPx = cellHeightPx, imeBottomPx = containerHeightPx),
        )
    }
}
