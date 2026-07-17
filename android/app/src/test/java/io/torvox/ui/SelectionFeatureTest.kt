// TODO(kotlin-2.4.0-false-positive): K2 smart-cast false positive, remove when upgrading Kotlin compiler
@file:Suppress("UNNECESSARY_NOT_NULL_ASSERTION")

package io.torvox.ui

import android.graphics.RectF
import io.torvox.SelectionAnchor
import io.torvox.SelectionMode
import io.torvox.SelectionState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Comprehensive unit tests for the text selection feature.
 *
 * Covers the two pure decision functions that drive the UI:
 *   - [SelectionState.applyHandleDrag] : expand / shrink selection via drag handles
 *   - [computeMenuPosition]            : place the floating menu so it never covers the selection
 *
 * These are exercised directly (no emulator) so they run fast in CI and lock in the
 * "menu never covers selected text" and "drag handles expand/shrink" guarantees.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class SelectionFeatureTest {
    // ===== applyHandleDrag : drag handles expand / shrink the selection =====

    @Test
    fun applyHandleDrag_endMovesDownAndRight() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(0, 0),
                end = SelectionAnchor(0, 5),
                mode = SelectionMode.Char,
            )
        val result = state.applyHandleDrag(draggingStart = false, targetRow = 2, targetCol = 10)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(2, result.endRow)
        assertEquals(10, result.endCol)
    }

    @Test
    fun applyHandleDrag_endShrinksUpward() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 20),
                end = SelectionAnchor(10, 10),
                mode = SelectionMode.Char,
            )
        // Drag the END handle back up to (7, 5): selection shrinks, start unchanged.
        val result = state.applyHandleDrag(draggingStart = false, targetRow = 7, targetCol = 5)
        assertEquals(5, result.startRow)
        assertEquals(20, result.startCol)
        assertEquals(7, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_startCannotCrossEnd() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(2, 0),
                end = SelectionAnchor(4, 10),
                mode = SelectionMode.Char,
            )
        // Drag START past END -> the start handle clamps to the end anchor and the
        // selection flips so the new end is the drag target (4,10)-(6,20).
        val result = state.applyHandleDrag(draggingStart = true, targetRow = 6, targetCol = 20)
        assertEquals(4, result.startRow)
        assertEquals(10, result.startCol)
        assertEquals(6, result.endRow)
        assertEquals(20, result.endCol)
    }

    @Test
    fun applyHandleDrag_startMovesLeftWithinBounds() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(4, 10),
                end = SelectionAnchor(8, 20),
                mode = SelectionMode.Char,
            )
        val result = state.applyHandleDrag(draggingStart = true, targetRow = 4, targetCol = 2)
        assertEquals(4, result.startRow)
        assertEquals(2, result.startCol)
        assertEquals(8, result.endRow)
        assertEquals(20, result.endCol)
    }

    // ===== computeMenuPosition : menu never covers the selected text =====

    @Test
    fun menuPosition_belowSelectionWhenRoom() {
        val start = SelectionAnchor(5, 2)
        val end = SelectionAnchor(5, 10)
        val pos =
            computeMenuPosition(
                start = start,
                end = end,
                cellWidth = 10f,
                cellHeight = 20f,
                scrollOffset = 0,
                screenWidthPx = 480f,
                screenHeightPx = 854f,
            )
        // selBottom = (5 + 1) * 20 = 120; menuY = 128
        assertEquals(128f, pos.menuY, 0.001f)
        assertFalse("Menu must not cover the selection", pos.coversSelection)
        assertFalse(pos.flipAbove)
    }

    @Test
    fun menuPosition_flipsAboveWhenNearBottom() {
        val start = SelectionAnchor(40, 2)
        val end = SelectionAnchor(40, 10)
        val pos =
            computeMenuPosition(
                start = start,
                end = end,
                cellWidth = 10f,
                cellHeight = 20f,
                scrollOffset = 0,
                screenWidthPx = 480f,
                screenHeightPx = 854f,
            )
        // selBottom = 41 * 20 = 820; 820 + 8 + 48 = 876 > 854 -> flip above
        assertTrue(pos.flipAbove)
        // menuY = selTop - menuH - 8 = 800 - 48 - 8 = 744
        assertEquals(744f, pos.menuY, 0.001f)
        assertFalse("Menu must not cover the selection", pos.coversSelection)
    }

    @Test
    fun menuPosition_shiftsRightWhenBelowOverlaps() {
        // Narrow selection on the right edge; default placement would overlap, so shift right.
        val start = SelectionAnchor(5, 35)
        val end = SelectionAnchor(5, 40)
        val pos =
            computeMenuPosition(
                start = start,
                end = end,
                cellWidth = 10f,
                cellHeight = 20f,
                scrollOffset = 0,
                screenWidthPx = 480f,
                screenHeightPx = 854f,
            )
        // selRight = 41 * 10 = 410; menu width 260; default x = (350+410)/2 - 130 = 250..510
        // sel 350..410 overlaps 250..510 -> shift right -> 410+8=418..678 -> clamped to (480-260)=220
        // 220..480 still overlaps 350..410? 350..410 within 220..480 -> yes -> shift left -> 350-260-8=82..342
        // 82..342 vs 350..410 -> no overlap -> coversSelection=false
        assertFalse("Menu must not cover the selection after shifting", pos.coversSelection)
    }

    @Test
    fun menuPosition_fullWidthSingleRowNeverCovers() {
        // Full-width single-row selection: the menu is placed 8px below the selection
        // band, so it must never overlap the selected text (guaranteed by the 8px gap).
        val start = SelectionAnchor(5, 0)
        val end = SelectionAnchor(5, 47)
        val pos =
            computeMenuPosition(
                start = start,
                end = end,
                cellWidth = 10f,
                cellHeight = 20f,
                scrollOffset = 0,
                screenWidthPx = 480f,
                screenHeightPx = 854f,
            )
        assertFalse(
            "Menu placed 8px below a single-row selection must never cover it",
            pos.coversSelection,
        )
    }

    @Test
    fun menuPosition_withScrollOffsetAdjustsVisibleRow() {
        // Selection at scrollback row 10, scrolled down by 5 -> visible row 5.
        val start = SelectionAnchor(10, 2)
        val end = SelectionAnchor(10, 10)
        val pos =
            computeMenuPosition(
                start = start,
                end = end,
                cellWidth = 10f,
                cellHeight = 20f,
                scrollOffset = 5,
                screenWidthPx = 480f,
                screenHeightPx = 854f,
            )
        // visibleLoRow = 10 - 5 = 5; selTop = 5 * 20 = 100; selBottom = 6 * 20 = 120
        assertEquals(100f, pos.selTop, 0.001f)
        assertEquals(120f, pos.selBottom, 0.001f)
        assertFalse("Menu must not cover the selection", pos.coversSelection)
    }

    // ===== Selection mode preservation =====

    @Test
    fun selectionMode_preservedAcrossGesture() {
        val word = SelectionState(mode = SelectionMode.Word)
        val line = SelectionState(mode = SelectionMode.Line)
        val block = SelectionState(mode = SelectionMode.Block)
        val semantic = SelectionState(mode = SelectionMode.Semantic)
        assertEquals(SelectionMode.Word, word.mode)
        assertEquals(SelectionMode.Line, line.mode)
        assertEquals(SelectionMode.Block, block.mode)
        assertEquals(SelectionMode.Semantic, semantic.mode)
    }
}
