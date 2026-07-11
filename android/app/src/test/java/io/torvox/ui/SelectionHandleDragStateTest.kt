package io.torvox.ui

import io.torvox.HandleDragResult
import io.torvox.SelectionAnchor
import io.torvox.SelectionMode
import io.torvox.SelectionState
import org.junit.Assert.assertEquals
import org.junit.Test

class SelectionHandleDragStateTest {
    private fun state(
        startRow: Int = 0,
        startCol: Int = 2,
        endRow: Int = 0,
        endCol: Int = 5,
    ): SelectionState = SelectionState(
        active = true,
        dragging = true,
        start = SelectionAnchor(startRow, startCol),
        end = SelectionAnchor(endRow, endCol),
        mode = SelectionMode.Char,
    )

    @Test
    fun applyHandleDrag_startMovesEarlier() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_startMovesLaterButBeforeEnd() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 3)
        assertEquals(0, result.startRow)
        assertEquals(3, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun applyHandleDrag_startCrossesEnd_flipsPoints() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 7)
        assertEquals(0, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(7, result.endCol)
    }

    @Test
    fun applyHandleDrag_startCrossesEndLaterRow_flipsPoints() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 1, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(1, result.endRow)
        assertEquals(0, result.endCol)
    }

    @Test
    fun applyHandleDrag_endMovesLater() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 9)
        assertEquals(0, result.startRow)
        assertEquals(2, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(9, result.endCol)
    }

    @Test
    fun applyHandleDrag_endCrossesStartEarlierRow_flipsPoints() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(2, result.endCol)
    }

    @Test
    fun applyHandleDrag_endCrossesStartLaterCol_flipsPoints() {
        val s = state()
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 1)
        assertEquals(0, result.startRow)
        assertEquals(1, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(2, result.endCol)
    }

    @Test
    fun applyHandleDrag_startDragOnSameRow() {
        val s = state(startRow = 0, startCol = 3, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 5)
        assertEquals(0, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(10, result.endCol)
    }

    @Test
    fun applyHandleDrag_endDragOnSameRow() {
        val s = state(startRow = 0, startCol = 3, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 7)
        assertEquals(0, result.startRow)
        assertEquals(3, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(7, result.endCol)
    }

    @Test
    fun applyHandleDrag_multiRowStartDraggedBack() {
        val s = state(startRow = 1, startCol = 0, endRow = 3, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 1, targetCol = 0)
        assertEquals(1, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(3, result.endRow)
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
}
