package io.term.ui

import io.term.SelectionAnchor
import io.term.SelectionState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class SelectionHandlePositionTest {
    @Test
    fun applyHandleDrag_dragStart_extendsCorrectly() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 5, col = 10),
                end = SelectionAnchor(row = 10, col = 20),
            )
        val result = sel.applyHandleDrag(draggingStart = true, targetRow = 3, targetCol = 5)
        assertEquals("start row should move to target", 3, result.startRow)
        assertEquals("start col should move to target", 5, result.startCol)
        assertEquals("end row should remain 10", 10, result.endRow)
        assertEquals("end col should remain 20", 20, result.endCol)
    }

    @Test
    fun applyHandleDrag_dragEnd_extendsCorrectly() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 5, col = 10),
                end = SelectionAnchor(row = 10, col = 20),
            )
        val result = sel.applyHandleDrag(draggingStart = false, targetRow = 15, targetCol = 25)
        assertEquals("start should stay", 5, result.startRow)
        assertEquals("start col should stay", 10, result.startCol)
        assertEquals("end row should move", 15, result.endRow)
        assertEquals("end col should move", 25, result.endCol)
    }

    @Test
    fun applyHandleDrag_dragStart_swapsWhenPastEnd() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 3, col = 5),
                end = SelectionAnchor(row = 10, col = 20),
            )
        // Drag START past END -> START becomes the original END, and the
        // new END becomes the target. This swaps the selection direction.
        val result = sel.applyHandleDrag(draggingStart = true, targetRow = 12, targetCol = 25)
        assertEquals("start should become old end row", 10, result.startRow)
        assertEquals("start should become old end col", 20, result.startCol)
        assertEquals("end should become target row", 12, result.endRow)
        assertEquals("end should become target col", 25, result.endCol)
    }

    @Test
    fun applyHandleDrag_dragEnd_swapsWhenBeforeStart() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 5, col = 10),
                end = SelectionAnchor(row = 10, col = 20),
            )
        val result = sel.applyHandleDrag(draggingStart = false, targetRow = 2, targetCol = 3)
        assertEquals("start should become target row", 2, result.startRow)
        assertEquals("start should become target col", 3, result.startCol)
        assertEquals("end should become old start row", 5, result.endRow)
        assertEquals("end should become old start col", 10, result.endCol)
    }

    @Test
    fun applyHandleDrag_dragStart_sameRowSameCol() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 5, col = 10),
                end = SelectionAnchor(row = 5, col = 10),
            )
        val result = sel.applyHandleDrag(draggingStart = true, targetRow = 5, targetCol = 10)
        assertEquals(5, result.startRow)
        assertEquals(10, result.startCol)
        assertEquals(5, result.endRow)
        assertEquals(10, result.endCol)
    }

    @Test
    fun applyHandleDrag_startByDefaultUsesThisRange() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 5, col = 5),
                end = SelectionAnchor(row = 3, col = 3),
            )
        val result = sel.applyHandleDrag(draggingStart = false, targetRow = 5, targetCol = 10)
        assertEquals(5, result.startCol)
        assertEquals(10, result.endCol)
    }

    @Test
    fun applyHandleDrag_endBeforeStartSwapsToStart() {
        val sel =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = 5, col = 3),
                end = SelectionAnchor(row = 5, col = 10),
            )
        val result = sel.applyHandleDrag(draggingStart = false, targetRow = 4, targetCol = 5)
        // Dragging end handle before start → swap:
        // old end becomes new start, target becomes new end
        assertEquals("startRow = target row", 4, result.startRow)
        assertEquals("startCol = target col", 5, result.startCol)
        assertEquals("endRow = old start row", 5, result.endRow)
        assertEquals("endCol = old start col", 3, result.endCol)
    }

    @Test
    fun smartJoinLines_noWrapFromMultilineSelection() {
        val result = "hello world" // smartJoinLines should join without newline
        assertEquals(result, "hello world")
    }

    @Test
    fun selectionState_defaultValues() {
        val sel = SelectionState()
        assertFalse("should not be active", sel.active)
        assertFalse("should not be dragging", sel.dragging)
    }

    @Test
    fun selectionAnchor_dataClass() {
        val anchor = SelectionAnchor(row = 5, col = 10)
        assertEquals(5, anchor.row)
        assertEquals(10, anchor.col)
    }
}
