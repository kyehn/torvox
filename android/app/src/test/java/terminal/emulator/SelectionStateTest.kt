package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure selection-state helpers: handle-drag crossing flips. No native
 * engine involved — the rules are exercised directly on [SelectionState].
 */
class SelectionStateTest {

    private fun selection(startRow: Int = 0, startCol: Int = 0, endRow: Int = 0, endCol: Int = 10) = SelectionState(
        active = true,
        start = SelectionAnchor(startRow, startCol),
        end = SelectionAnchor(endRow, endCol),
    )

    // ── applyHandleDrag crossing flip ────────────────────────────────────

    @Test
    fun `dragging start past end swaps ownership`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 12)
        // START swept past END: new start pins at the old END, END follows the finger.
        assertEquals(HandleDragResult(0, 10, 0, 12, false), result)
    }

    @Test
    fun `dragging start below end swaps ownership across rows`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 2, endCol = 5)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 3, targetCol = 0)
        assertEquals(HandleDragResult(2, 5, 3, 0, false), result)
    }

    @Test
    fun `dragging end before start swaps ownership`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 0)
        assertEquals(HandleDragResult(0, 0, 0, 2, true), result)
    }

    @Test
    fun `normal start drag keeps ordering`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 5)
        assertEquals(HandleDragResult(0, 5, 0, 10, true), result)
    }

    @Test
    fun `normal end drag keeps ordering`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 12)
        assertEquals(HandleDragResult(0, 2, 0, 12, false), result)
    }

    @Test
    fun `landing exactly on anchor does not flip`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val startLandsOnEnd = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 10)
        assertEquals(HandleDragResult(0, 10, 0, 10, true), startLandsOnEnd)
        val endLandsOnStart = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 2)
        assertEquals(HandleDragResult(0, 2, 0, 2, false), endLandsOnStart)
    }

    @Test
    fun `cross then drag back follows flipped handle`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val crossed = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 12)
        assertEquals(false, crossed.draggingStart)
        val moved =
            selection(
                startRow = crossed.startRow,
                startCol = crossed.startCol,
                endRow = crossed.endRow,
                endCol = crossed.endCol,
            )
        val back = moved.applyHandleDrag(draggingStart = crossed.draggingStart, targetRow = 0, targetCol = 11)
        assertEquals(HandleDragResult(0, 10, 0, 11, false), back)
    }
}
