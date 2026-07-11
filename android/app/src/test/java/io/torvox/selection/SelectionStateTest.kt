package io.torvox.selection

import io.torvox.HandleDragResult
import io.torvox.SelectionAnchor
import io.torvox.SelectionMode
import io.torvox.SelectionState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SelectionStateTest {
    private fun state(
        startRow: Int = 0,
        startCol: Int = 2,
        endRow: Int = 0,
        endCol: Int = 5,
        mode: SelectionMode = SelectionMode.Char,
    ): SelectionState = SelectionState(
        active = true,
        dragging = true,
        start = SelectionAnchor(startRow, startCol),
        end = SelectionAnchor(endRow, endCol),
        mode = mode,
    )

    @Test
    fun selectionState_creationDefaults() {
        val s = SelectionState()
        assertFalse(s.active)
        assertFalse(s.dragging)
        assertNull(s.start)
        assertNull(s.end)
        assertEquals(SelectionMode.Char, s.mode)
        assertEquals("", s.selectedText)
    }

    @Test
    fun selectionState_creationWithValues() {
        val s =
            SelectionState(
                active = true,
                dragging = true,
                start = SelectionAnchor(1, 2),
                end = SelectionAnchor(3, 4),
                mode = SelectionMode.Word,
                selectedText = "hello",
            )
        assertTrue(s.active)
        assertTrue(s.dragging)
        assertEquals(1, s.start!!.row)
        assertEquals(2, s.start!!.col)
        assertEquals(3, s.end!!.row)
        assertEquals(4, s.end!!.col)
        assertEquals(SelectionMode.Word, s.mode)
        assertEquals("hello", s.selectedText)
    }

    @Test
    fun applyHandleDrag_startDraggedForward_sameRow() {
        val s = state(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 5)
        assertEquals(0, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(10, result.endCol)
    }

    @Test
    fun applyHandleDrag_startDraggedBackward_pastEnd_swaps() {
        val s = state(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 15)
        assertEquals(0, result.startRow)
        assertEquals(10, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(15, result.endCol)
    }

    @Test
    fun applyHandleDrag_endDraggedForward() {
        val s = state(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 20)
        assertEquals(0, result.startRow)
        assertEquals(2, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(20, result.endCol)
    }

    @Test
    fun applyHandleDrag_endDraggedBackward_pastStart_swaps() {
        val s = state(startRow = 0, startCol = 5, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 2)
        assertEquals(0, result.startRow)
        assertEquals(2, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_singleCellSelection() {
        val s = state(startRow = 3, startCol = 7, endRow = 3, endCol = 7)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 3, targetCol = 12)
        assertEquals(3, result.startRow)
        assertEquals(7, result.startCol)
        assertEquals(3, result.endRow)
        assertEquals(12, result.endCol)
    }

    @Test
    fun handleDragResult_equality() {
        val a = HandleDragResult(0, 1, 2, 3)
        val b = HandleDragResult(0, 1, 2, 3)
        val c = HandleDragResult(0, 0, 2, 3)
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
        assertNotEquals(a, c)
    }

    @Test
    fun applyHandleDrag_startCrossesEnd_laterRow_swaps() {
        val s = state(startRow = 1, startCol = 0, endRow = 3, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 4, targetCol = 2)
        assertEquals(3, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(4, result.endRow)
        assertEquals(2, result.endCol)
    }

    @Test
    fun applyHandleDrag_endCrossesStart_earlierRow_swaps() {
        val s = state(startRow = 2, startCol = 3, endRow = 4, endCol = 1)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 1, targetCol = 0)
        assertEquals(1, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(2, result.endRow)
        assertEquals(3, result.endCol)
    }

    @Test
    fun applyHandleDrag_startAtSamePositionAsEnd() {
        val s = state(startRow = 0, startCol = 5, endRow = 0, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 8)
        assertEquals(0, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(8, result.endCol)
    }

    @Test
    fun applyHandleDrag_endAtSamePositionAsStart_reverse() {
        val s = state(startRow = 0, startCol = 5, endRow = 0, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 2)
        assertEquals(0, result.startRow)
        assertEquals(2, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_multiRow_startStaysWithin() {
        val s = state(startRow = 1, startCol = 3, endRow = 3, endCol = 8)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 2, targetCol = 5)
        assertEquals(2, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(3, result.endRow)
        assertEquals(8, result.endCol)
    }

    @Test
    fun applyHandleDrag_startCrossesEnd_equalCol_laterRow_swaps() {
        val s = state(startRow = 1, startCol = 5, endRow = 3, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 4, targetCol = 5)
        assertEquals(3, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(4, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_endCrossesStart_equalCol_earlierRow_swaps() {
        val s = state(startRow = 3, startCol = 5, endRow = 5, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 2, targetCol = 5)
        assertEquals(2, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(3, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_startAtCol0_dragLeft_noWrap() {
        val s = state(startRow = 0, startCol = 0, endRow = 0, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_endAtLastCol_dragRight_noWrap() {
        val s = state(startRow = 0, startCol = 3, endRow = 0, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 5)
        assertEquals(0, result.startRow)
        assertEquals(3, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_nullStart_returnsDefault() {
        val s = SelectionState(active = true, end = SelectionAnchor(0, 5))
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(0, result.endCol)
    }

    @Test
    fun applyHandleDrag_nullEnd_returnsDefault() {
        val s = SelectionState(active = true, start = SelectionAnchor(0, 2))
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 5)
        assertEquals(0, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_inactiveSelection_returnsDefault() {
        val s = SelectionState(active = false)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(0, result.endCol)
    }

    @Test
    fun selectionAnchor_dataClass() {
        val a = SelectionAnchor(5, 10)
        val b = SelectionAnchor(5, 10)
        val c = SelectionAnchor(5, 11)
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
        assertNotEquals(a, c)
        assertEquals("SelectionAnchor(row=5, col=10)", a.toString())
    }

    @Test
    fun selectionMode_enumValues() {
        assertEquals(5, SelectionMode.entries.size)
        assertTrue(SelectionMode.entries.contains(SelectionMode.Char))
        assertTrue(SelectionMode.entries.contains(SelectionMode.Word))
        assertTrue(SelectionMode.entries.contains(SelectionMode.Line))
        assertTrue(SelectionMode.entries.contains(SelectionMode.Block))
        assertTrue(SelectionMode.entries.contains(SelectionMode.Semantic))
    }

    @Test
    fun selectionState_copyPreservesMode() {
        val original = SelectionState(mode = SelectionMode.Word)
        val copy = original.copy(active = true)
        assertEquals(SelectionMode.Word, copy.mode)
    }

    @Test
    fun selectionState_selectedTextDefaultsEmpty() {
        val s = SelectionState()
        assertEquals("", s.selectedText)
    }
}
