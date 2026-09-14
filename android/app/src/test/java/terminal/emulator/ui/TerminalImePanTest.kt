package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Unit tests for the IME-follow cursor pan formula (no reflow, minimal shift). */
class TerminalImePanTest {

    // cellH=60px, boxH=2100px, ime=1100px, bar=240px → visible content 760px.
    @Test
    fun pan_zero_when_keyboard_closed() {
        assertEquals(0, computeTerminalPanPx(cursorRow = 30, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 0, barPx = 240))
    }

    @Test
    fun pan_zero_for_fresh_prompt_at_top() {
        // Single prompt row at the top stays put — the old full-height pan
        // pushed it off-screen and showed a black window.
        assertEquals(0, computeTerminalPanPx(cursorRow = 0, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240))
    }

    @Test
    fun pan_zero_for_visible_mid_screen_cursor() {
        assertEquals(0, computeTerminalPanPx(cursorRow = 10, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240))
    }

    @Test
    fun pan_partial_for_covered_cursor() {
        // Cursor bottom 1260px, visible 760px → shift exactly 500px.
        assertEquals(500, computeTerminalPanPx(cursorRow = 20, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240))
    }

    @Test
    fun pan_full_for_bottom_cursor() {
        // 31-row grid ((2100-240)/60), cursor on the last row → full shift.
        assertEquals(1100, computeTerminalPanPx(cursorRow = 30, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240))
    }

    @Test
    fun pan_holds_when_cursor_hidden() {
        // -1 (scrolled into history): caller keeps the previous pan.
        assertNull(computeTerminalPanPx(cursorRow = -1, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240))
    }

    @Test
    fun pan_full_when_metrics_unknown() {
        assertEquals(1100, computeTerminalPanPx(cursorRow = 5, cellHeightPx = 0f, boxHeightPx = 2100, imePx = 1100, barPx = 240))
    }
}
