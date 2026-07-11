package io.torvox.ui

import io.torvox.HandleDragResult
import io.torvox.SelectionAnchor
import io.torvox.SelectionMode
import io.torvox.SelectionState
import org.junit.Assert.assertEquals
import org.junit.Test

class SelectionCrossoverTest {
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
    fun dragStartPastEnd_sameRow_flips() {
        val s = state(startRow = 0, startCol = 3, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 15)
        assertEquals("start should pin to old end.col=10 after flip", 0, result.startRow)
        assertEquals(10, result.startCol)
        assertEquals("end should move to target col=15", 0, result.endRow)
        assertEquals(15, result.endCol)
    }

    @Test
    fun dragEndBeforeStart_sameRow_flips() {
        val s = state(startRow = 0, startCol = 5, endRow = 0, endCol = 10)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 2)
        assertEquals("start should pin to old start.col=5", 0, result.startRow)
        assertEquals(2, result.startCol)
        assertEquals("end should become old start", 0, result.endRow)
        assertEquals(5, result.endCol)
    }

    @Test
    fun dragStartPastEnd_multiRow_flipsRows() {
        val s = state(startRow = 1, startCol = 0, endRow = 3, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 4, targetCol = 2)
        assertEquals(3, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(4, result.endRow)
        assertEquals(2, result.endCol)
    }

    @Test
    fun dragEndBeforeStart_multiRow_flipsRows() {
        val s = state(startRow = 2, startCol = 3, endRow = 4, endCol = 1)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 1, targetCol = 0)
        assertEquals(1, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(2, result.endRow)
        assertEquals(3, result.endCol)
    }

    @Test
    fun dragStartPastEnd_thenDragBack_restoresOrder() {
        val s = state(startRow = 0, startCol = 3, endRow = 0, endCol = 10)
        val first: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 15)
        assertEquals(0, first.startRow)
        assertEquals(10, first.startCol)
        assertEquals(0, first.endRow)
        assertEquals(15, first.endCol)
    }

    @Test
    fun crossoverAtRowBoundary_startRow3endRow4_dragStartToRow5() {
        val s = state(startRow = 3, startCol = 0, endRow = 4, endCol = 5)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = true, targetRow = 5, targetCol = 2)
        assertEquals(4, result.startRow)
        assertEquals(5, result.startCol)
        assertEquals(5, result.endRow)
        assertEquals(2, result.endCol)
    }

    @Test
    fun crossoverAtRowBoundary_endRow5startRow4_dragEndToRow2() {
        val s = state(startRow = 4, startCol = 3, endRow = 5, endCol = 1)
        val result: HandleDragResult = s.applyHandleDrag(draggingStart = false, targetRow = 2, targetCol = 0)
        assertEquals(2, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(4, result.endRow)
        assertEquals(3, result.endCol)
    }
}
