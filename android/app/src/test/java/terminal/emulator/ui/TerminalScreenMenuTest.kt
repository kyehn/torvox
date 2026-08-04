package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Unit tests for the selection context-menu placement logic. */
@RunWith(RobolectricTestRunner::class)
class TerminalScreenMenuTest {

    private fun sel(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
    ) = terminal.emulator.SelectionAnchor(row = startRow, col = startCol) to
        terminal.emulator.SelectionAnchor(row = endRow, col = endCol)

    @Test
    fun menu_below_selection_when_space() {
        val (start, end) = sel(2, 0, 2, 4)
        val pos = computeMenuPosition(start, end, 100f, 50f, 0, 1080f, 2209f, 40f)
        // Selection: x 0-500, y 100-150 (row 2 visible). Menu below + handle gap.
        assertEquals(0f, pos.selLeft, 0.1f)
        assertEquals(150f, pos.selBottom, 0.1f)
        assertTrue("menu must be below selection", pos.menuY > pos.selBottom)
        assertFalse("menu must not cover selection", pos.coversSelection)
    }

    @Test
    fun menu_flips_above_when_no_room_below() {
        // Selection at the very bottom of the screen: below-placement would
        // overflow, so the menu must flip above the selection.
        val (start, end) = sel(43, 0, 43, 4)
        val pos = computeMenuPosition(start, end, 100f, 50f, 0, 1080f, 2209f, 40f)
        assertTrue("menu must flip above", pos.flipAbove)
        assertTrue("flipped menu above selection", pos.menuY + pos.menuH <= pos.selTop)
    }

    @Test
    fun menu_centered_horizontally_on_selection() {
        val (start, end) = sel(1, 2, 1, 6)
        val pos = computeMenuPosition(start, end, 100f, 50f, 0, 1080f, 2209f, 40f)
        val selMid = (pos.selLeft + pos.selRight) / 2f
        val menuMid = pos.menuX + pos.menuW / 2f
        assertEquals("menu centered on selection", selMid, menuMid, 5f)
    }

    @Test
    fun menu_stays_on_screen() {
        val (start, end) = sel(0, 0, 0, 0)
        val pos = computeMenuPosition(start, end, 100f, 50f, 0, 1080f, 2209f, 40f)
        assertTrue("menuX >= 0", pos.menuX >= 0f)
        assertTrue("menuX + menuW <= screen", pos.menuX + pos.menuW <= 1080f)
        assertTrue("menuY >= 0", pos.menuY >= 0f)
        assertTrue("menuY + menuH <= screen", pos.menuY + pos.menuH <= 2209f)
    }

    @Test
    fun menu_handles_scroll_offset() {
        // Scrolled content: selection grid row 5 with scrollOffset 2 -> visible row 3.
        val (start, end) = sel(5, 0, 5, 3)
        val pos = computeMenuPosition(start, end, 100f, 50f, 2, 1080f, 2209f, 40f)
        assertEquals("selTop uses visible row", 150f, pos.selTop, 0.1f)
    }

    @Test
    fun menu_paste_only_hugs_selection() {
        val (start, end) = sel(1, 1, 1, 1)
        val withHandle = computeMenuPosition(start, end, 100f, 50f, 0, 1080f, 2209f, 40f, pasteOnly = false)
        val pasteOnly = computeMenuPosition(start, end, 100f, 50f, 0, 1080f, 2209f, 40f, pasteOnly = true)
        assertTrue(
            "paste-only menu hugs the selection closer (no handle gap)",
            pasteOnly.menuY < withHandle.menuY,
        )
    }

    @Test
    fun menu_adapts_to_narrow_screen() {
        // 360dp wide display: menu width must not exceed screen - 16px.
        val (start, end) = sel(1, 0, 1, 4)
        val pos = computeMenuPosition(start, end, 19.2f, 45.6f, 0, 480f, 854f, 40f)
        assertTrue("menu fits narrow screen", pos.menuW <= 464f)
        assertTrue("menu stays on screen", pos.menuX + pos.menuW <= 480f)
        assertFalse("menu must not cover selection", pos.coversSelection)
    }

    @Test
    fun menu_fits_480px_screen() {
        val (start, end) = sel(3, 0, 3, 9)
        val pos = computeMenuPosition(start, end, 19.2f, 45.6f, 0, 480f, 854f, 40f)
        assertTrue("menuX >= 0", pos.menuX >= 0f)
        assertTrue("menu right edge on screen", pos.menuX + pos.menuW <= 480f)
        assertTrue("menu below selection", pos.menuY > pos.selBottom)
    }
}
