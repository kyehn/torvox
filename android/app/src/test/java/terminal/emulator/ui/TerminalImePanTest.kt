package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Unit tests for the IME-follow cursor pan formula (no reflow, minimal shift). */
class TerminalImePanTest {

    // cellH=60px, boxH=2100px, ime=1100px, bar=240px → visible content 760px.
    @Test
    fun pan_zero_when_keyboard_closed() {
        assertEquals(
            0,
            computeTerminalPanPx(cursorRow = 30, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 0, barPx = 240),
        )
    }

    @Test
    fun pan_zero_for_fresh_prompt_at_top() {
        // Single prompt row at the top stays put — the old full-height pan
        // pushed it off-screen and showed a black window.
        assertEquals(
            0,
            computeTerminalPanPx(cursorRow = 0, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_zero_for_visible_mid_screen_cursor() {
        assertEquals(
            0,
            computeTerminalPanPx(cursorRow = 10, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_partial_for_covered_cursor() {
        // Cursor bottom 1260px, visible 760px → shift exactly 500px.
        assertEquals(
            500,
            computeTerminalPanPx(cursorRow = 20, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_full_for_bottom_cursor() {
        // 31-row grid ((2100-240)/60), cursor on the last row → full shift.
        assertEquals(
            1100,
            computeTerminalPanPx(cursorRow = 30, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_holds_when_cursor_hidden() {
        // -1 (scrolled into history): caller keeps the previous pan.
        assertNull(
            computeTerminalPanPx(cursorRow = -1, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_full_when_metrics_unknown() {
        assertEquals(
            1100,
            computeTerminalPanPx(cursorRow = 5, cellHeightPx = 0f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_grows_with_live_frame_during_show() {
        // 同一光标行，动画中间帧（ime=550）位移小于定居值（ime=1100）：
        // 调用方必须逐帧喂 live 值，冻结 settled 值会在此断言失败。
        assertEquals(
            250,
            computeTerminalPanPx(cursorRow = 25, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 550, barPx = 240),
        )
        assertEquals(
            800,
            computeTerminalPanPx(cursorRow = 25, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_shrinks_when_cursor_moves_up() {
        // 退格/回车后光标上移：同一键盘高度下位移必须跟随缩小（20 行→500px，12 行→20px）。
        assertEquals(
            20,
            computeTerminalPanPx(cursorRow = 12, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }

    @Test
    fun pan_follows_live_value_during_hide() {
        // 隐藏动画中间帧不得冻结在定居值 1100：live ime=100 时位移必须已回落到 100。
        assertEquals(
            100,
            computeTerminalPanPx(cursorRow = 30, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 100, barPx = 240),
        )
    }

    @Test
    fun pan_never_exceeds_keyboard_height() {
        // 光标远超可视区时位移钳制在键盘高度内，不向上超调遮挡更多内容。
        assertEquals(
            1100,
            computeTerminalPanPx(cursorRow = 100, cellHeightPx = 60f, boxHeightPx = 2100, imePx = 1100, barPx = 240),
        )
    }
}
