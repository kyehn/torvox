package io.term.selection

import io.term.HandleDragResult
import io.term.SelectionAnchor
import io.term.SelectionMode
import io.term.SelectionState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SelectionStateTest {
    @Test
    fun `initial state is not active`() {
        val state = SelectionState()
        assertFalse(state.active)
        assertFalse(state.dragging)
        assertNull(state.start)
        assertNull(state.end)
        assertEquals(SelectionMode.Char, state.mode)
        assertEquals("", state.selectedText)
    }

    @Test
    fun `active state with anchors`() {
        val start = SelectionAnchor(5, 10)
        val end = SelectionAnchor(5, 10)
        val state =
            SelectionState(
                active = true,
                start = start,
                end = end,
                mode = SelectionMode.Word,
            )
        assertTrue(state.active)
        assertEquals(5, start.row)
        assertEquals(10, start.col)
        assertEquals(5, end.row)
        assertEquals(10, end.col)
    }

    @Test
    fun `applyHandleDrag right handle extends right`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 10),
                end = SelectionAnchor(5, 10),
            )
        val result = state.applyHandleDrag(draggingStart = false, targetRow = 5, targetCol = 15)
        assertEquals(5, result.startRow)
        assertEquals(10, result.startCol)
        assertEquals(5, result.endRow)
        assertEquals(15, result.endCol)
    }

    @Test
    fun `applyHandleDrag left handle shrinks from left`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 10),
                end = SelectionAnchor(5, 15),
            )
        val result = state.applyHandleDrag(draggingStart = true, targetRow = 5, targetCol = 12)
        assertEquals(5, result.startRow)
        assertEquals(12, result.startCol)
        assertEquals(5, result.endRow)
        assertEquals(15, result.endCol)
    }

    @Test
    fun `applyHandleDrag right handle past start flips anchors`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 10),
                end = SelectionAnchor(5, 15),
            )
        val result = state.applyHandleDrag(draggingStart = false, targetRow = 5, targetCol = 8)
        assertEquals(5, result.startRow)
        assertEquals(8, result.startCol)
        assertEquals(5, result.endRow)
        assertEquals(10, result.endCol)
    }

    @Test
    fun `applyHandleDrag left handle past end flips anchors`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 10),
                end = SelectionAnchor(5, 15),
            )
        val result = state.applyHandleDrag(draggingStart = true, targetRow = 5, targetCol = 18)
        assertEquals(5, result.startRow)
        assertEquals(15, result.startCol)
        assertEquals(5, result.endRow)
        assertEquals(18, result.endCol)
    }

    @Test
    fun `applyHandleDrag multi-row right handle`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(2, 5),
                end = SelectionAnchor(3, 5),
            )
        val result = state.applyHandleDrag(draggingStart = false, targetRow = 4, targetCol = 3)
        assertEquals(2, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(4, result.endRow)
        assertEquals(3, result.endCol)
    }

    @Test
    fun `applyHandleDrag multi-row crossover flips start and end`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(3, 10),
                end = SelectionAnchor(5, 5),
            )
        val result = state.applyHandleDrag(draggingStart = true, targetRow = 6, targetCol = 2)
        assertEquals(5, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(6, result.endRow)
        assertEquals(2, result.endCol)
    }

    @Test
    fun `applyHandleDrag no start or end returns target coords`() {
        val emptyState = SelectionState()
        val result = emptyState.applyHandleDrag(draggingStart = true, targetRow = 7, targetCol = 3)
        assertEquals(HandleDragResult(7, 3, 7, 3), result)
    }

    @Test
    fun `selection range on same row`() {
        val start = SelectionAnchor(0, 3)
        val end = SelectionAnchor(0, 8)
        val state =
            SelectionState(
                active = true,
                start = start,
                end = end,
            )
        assertEquals(0, start.row)
        assertEquals(3, start.col)
        assertEquals(0, end.row)
        assertEquals(8, end.col)
    }

    @Test
    fun `selection range across multiple rows`() {
        val start = SelectionAnchor(2, 15)
        val end = SelectionAnchor(5, 7)
        val state =
            SelectionState(
                active = true,
                start = start,
                end = end,
            )
        assertEquals(2, start.row)
        assertEquals(15, start.col)
        assertEquals(5, end.row)
        assertEquals(7, end.col)
    }

    @Test
    fun `copy of selection state preserves values`() {
        val origStart = SelectionAnchor(3, 7)
        val origEnd = SelectionAnchor(3, 12)
        val state =
            SelectionState(
                active = true,
                start = origStart,
                end = origEnd,
                mode = SelectionMode.Line,
                selectedText = "hello",
            )
        val copy = state.copy(selectedText = "hello world")
        assertTrue(copy.active)
        assertEquals("hello world", copy.selectedText)
        assertEquals(3, copy.start!!.row)
    }

    @Test
    fun `selection with semantic mode`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(0, 0),
                end = SelectionAnchor(0, 20),
                mode = SelectionMode.Semantic,
            )
        assertEquals(SelectionMode.Semantic, state.mode)
    }

    @Test
    fun `selection with block mode`() {
        val state =
            SelectionState(
                active = true,
                start = SelectionAnchor(2, 5),
                end = SelectionAnchor(5, 10),
                mode = SelectionMode.Block,
            )
        assertEquals(SelectionMode.Block, state.mode)
    }
}
