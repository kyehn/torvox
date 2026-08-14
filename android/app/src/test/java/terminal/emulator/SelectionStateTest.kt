package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * Pure selection-state helpers: arrow-key anchor movement and
 * handle-drag crossing flips. No native engine involved — the rules are
 * exercised directly on [SelectionState].
 */
class SelectionStateTest {

    private fun selection(
        startRow: Int = 0,
        startCol: Int = 0,
        endRow: Int = 0,
        endCol: Int = 10,
        active: Boolean = true,
    ) = SelectionState(
        active = active,
        start = SelectionAnchor(startRow, startCol),
        end = SelectionAnchor(endRow, endCol),
    )

    // ── moveSelectionAnchorBy ────────────────────────────────────────────

    @Test
    fun `right arrow moves start one column`() {
        val s = selection(startCol = 2, endCol = 10)
        val moved = s.moveSelectionAnchorBy(deltaRow = 0, deltaCol = 1, maxRow = 23, maxCol = 79)
        assertEquals(SelectionAnchor(0, 3), moved.start)
        assertEquals(SelectionAnchor(0, 10), moved.end)
    }

    @Test
    fun `right arrow clamps at grid edge`() {
        val s = selection(startCol = 78, endCol = 79)
        val moved = s.moveSelectionAnchorBy(deltaRow = 0, deltaCol = 5, maxRow = 23, maxCol = 79)
        assertEquals(SelectionAnchor(0, 79), moved.start)
    }

    @Test
    fun `left arrow never goes below zero`() {
        val s = selection(startCol = 1, endCol = 10)
        val moved = s.moveSelectionAnchorBy(deltaRow = 0, deltaCol = -5, maxRow = 23, maxCol = 79)
        assertEquals(SelectionAnchor(0, 0), moved.start)
    }

    @Test
    fun `down arrow moves start one row`() {
        val s = selection(startRow = 2, endRow = 10)
        val moved = s.moveSelectionAnchorBy(deltaRow = 1, deltaCol = 0, maxRow = 23, maxCol = 79)
        assertEquals(SelectionAnchor(3, 0), moved.start)
    }

    @Test
    fun `down arrow clamps at last row`() {
        val s = selection(startRow = 22, endRow = 23)
        val moved = s.moveSelectionAnchorBy(deltaRow = 5, deltaCol = 0, maxRow = 23, maxCol = 79)
        assertEquals(SelectionAnchor(23, 0), moved.start)
    }

    @Test
    fun `up arrow never goes above zero`() {
        val s = selection(startRow = 0, endRow = 10)
        val moved = s.moveSelectionAnchorBy(deltaRow = -1, deltaCol = 0, maxRow = 23, maxCol = 79)
        assertEquals(SelectionAnchor(0, 0), moved.start)
    }

    @Test
    fun `start never crosses end on the same row`() {
        val s = selection(startCol = 8, endCol = 10)
        val moved = s.moveSelectionAnchorBy(deltaRow = 0, deltaCol = 5, maxRow = 23, maxCol = 79)
        // Sweeping past END clamps just before it: same row → col = end.col - 1.
        assertEquals(SelectionAnchor(0, 9), moved.start)
        assertEquals(SelectionAnchor(0, 10), moved.end)
    }

    @Test
    fun `start never crosses below end`() {
        val s = selection(startRow = 0, endRow = 2, endCol = 5)
        val moved = s.moveSelectionAnchorBy(deltaRow = 3, deltaCol = 0, maxRow = 23, maxCol = 79)
        // Below END clamps to the row before it with col just before END's col.
        assertEquals(SelectionAnchor(1, 4), moved.start)
    }

    @Test
    fun `inactive selection is returned unchanged`() {
        val s = selection(active = false)
        assertSame(s, s.moveSelectionAnchorBy(deltaRow = 1, deltaCol = 0, maxRow = 23, maxCol = 79))
    }

    @Test
    fun `selection without anchors is returned unchanged`() {
        val s = SelectionState(active = true, start = null, end = null)
        assertSame(s, s.moveSelectionAnchorBy(deltaRow = 1, deltaCol = 0, maxRow = 23, maxCol = 79))
    }

    // ── applyHandleDrag crossing flip ────────────────────────────────────

    @Test
    fun `dragging start past end swaps ownership`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 12)
        // START swept past END: new start pins at the old END, END follows the finger.
        assertEquals(HandleDragResult(0, 10, 0, 12), result)
    }

    @Test
    fun `dragging start below end swaps ownership across rows`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 2, endCol = 5)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 3, targetCol = 0)
        assertEquals(HandleDragResult(2, 5, 3, 0), result)
    }

    @Test
    fun `dragging end before start swaps ownership`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 0)
        assertEquals(HandleDragResult(0, 0, 0, 2), result)
    }

    @Test
    fun `normal start drag keeps ordering`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 5)
        assertEquals(HandleDragResult(0, 5, 0, 10), result)
    }

    @Test
    fun `normal end drag keeps ordering`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 12)
        assertEquals(HandleDragResult(0, 2, 0, 12), result)
    }
}
