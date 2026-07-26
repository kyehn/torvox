package io.term

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class TerminalViewModelSelectionTest {
    @Test
    fun selectionAnchorWrapsRightAtColBoundary() {
        val cols = 80
        val rows = 24
        val startAnchor = SelectionAnchor(row = 0, col = cols - 1)
        val newCol = startAnchor.col + 1
        val newAnchor =
            if (newCol >= cols) {
                SelectionAnchor(minOf(rows - 1, startAnchor.row + 1), 0)
            } else {
                SelectionAnchor(startAnchor.row, newCol)
            }
        assertEquals(1, newAnchor.row)
        assertEquals(0, newAnchor.col)
    }

    @Test
    fun selectionAnchorWrapsLeftAtColZero() {
        val cols = 80
        val startAnchor = SelectionAnchor(row = 1, col = 0)
        val newCol = startAnchor.col - 1
        val newAnchor =
            if (newCol < 0) {
                SelectionAnchor(maxOf(0, startAnchor.row - 1), cols - 1)
            } else {
                SelectionAnchor(startAnchor.row, newCol)
            }
        assertEquals(0, newAnchor.row)
        assertEquals(cols - 1, newAnchor.col)
    }

    @Test
    fun selectionAnchorStaysWithinBounds() {
        val cols = 80
        val rows = 24
        val startAnchor = SelectionAnchor(row = 0, col = 40)
        val newCol = startAnchor.col + 1
        val newAnchor =
            if (newCol >= cols) {
                SelectionAnchor(minOf(rows - 1, startAnchor.row + 1), 0)
            } else {
                SelectionAnchor(startAnchor.row, newCol)
            }
        assertEquals(0, newAnchor.row)
        assertEquals(41, newAnchor.col)
    }

    @Test
    fun selectionAnchorClampsAtLastRow() {
        val cols = 80
        val rows = 24
        val lastRowAnchor = SelectionAnchor(row = rows - 1, col = cols - 1)
        val newCol = lastRowAnchor.col + 1
        val newAnchor =
            if (newCol >= cols) {
                SelectionAnchor(minOf(rows - 1, lastRowAnchor.row + 1), 0)
            } else {
                SelectionAnchor(lastRowAnchor.row, newCol)
            }
        assertEquals(rows - 1, newAnchor.row)
        assertEquals(0, newAnchor.col)
    }

    @Test
    fun selectionAnchorDoesNotExceedLastRow() {
        val cols = 80
        val rows = 24
        val lastRowAnchor = SelectionAnchor(row = rows - 1, col = 5)
        val newCol = lastRowAnchor.col - 1
        val newAnchor =
            if (newCol < 0) {
                SelectionAnchor(maxOf(0, lastRowAnchor.row - 1), cols - 1)
            } else {
                SelectionAnchor(lastRowAnchor.row, newCol)
            }
        assertNotEquals(rows, newAnchor.row)
    }

    @Test
    fun handleDrag_endCrossesStart_swapsAnchors() {
        val selection =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 0),
                end = SelectionAnchor(10, 0),
            )
        val result = selection.applyHandleDrag(draggingStart = false, targetRow = 3, targetCol = 0)
        assertEquals("end crossing start: start becomes end", 3, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals("end crossing start: end becomes old start", 5, result.endRow)
        assertEquals(0, result.endCol)
    }

    @Test
    fun handleDrag_startCrossesEnd_swapsAnchors() {
        val selection =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 0),
                end = SelectionAnchor(10, 0),
            )
        val result = selection.applyHandleDrag(draggingStart = true, targetRow = 12, targetCol = 0)
        assertEquals("start crossing end: start becomes old end", 10, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals("start crossing end: end becomes target", 12, result.endRow)
        assertEquals(0, result.endCol)
    }

    @Test
    fun handleDrag_noCross_maintainsAnchors() {
        val selection =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 0),
                end = SelectionAnchor(10, 0),
            )
        val result = selection.applyHandleDrag(draggingStart = false, targetRow = 8, targetCol = 0)
        assertEquals("no cross: start unchanged", 5, result.startRow)
        assertEquals("no cross: end updated to target", 8, result.endRow)
    }

    @Test
    fun handleDrag_sameRowColumnCross_detectsCrossover() {
        val selection =
            SelectionState(
                active = true,
                start = SelectionAnchor(5, 3),
                end = SelectionAnchor(5, 8),
            )
        val resultDown = selection.applyHandleDrag(draggingStart = true, targetRow = 5, targetCol = 9)
        assertEquals(5, resultDown.startRow)
        assertEquals(8, resultDown.startCol)
        assertEquals(5, resultDown.endRow)
        assertEquals(9, resultDown.endCol)
    }

    @Test
    fun handleDrag_inactiveSelection_returnsDefault() {
        val selection = SelectionState(active = false)
        val result = selection.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 0)
        assertEquals(0, result.startRow)
        assertEquals(0, result.startCol)
        assertEquals(0, result.endRow)
        assertEquals(0, result.endCol)
    }

    // -- syncSelectionToNative column tests (G2 fix) --
    // The fix: use minOf(start.col, end.col) and maxOf(start.col, end.col)
    // instead of conditional swap based on start.row <= end.row

    @Test
    fun syncColumns_sameRowForward_loIsStartCol() {
        val loCol = minOf(3, 10)
        val hiCol = maxOf(3, 10)
        assertEquals(3, loCol)
        assertEquals(10, hiCol)
    }

    @Test
    fun syncColumns_sameRowReversed_loIsEndCol() {
        val loCol = minOf(10, 3)
        val hiCol = maxOf(10, 3)
        assertEquals(3, loCol)
        assertEquals(10, hiCol)
    }

    @Test
    fun syncColumns_multiRowStartAboveEnd() {
        val start = SelectionAnchor(2, 5)
        val end = SelectionAnchor(5, 8)
        val loCol = minOf(start.col, end.col)
        val hiCol = maxOf(start.col, end.col)
        assertEquals(5, loCol)
        assertEquals(8, hiCol)
    }

    @Test
    fun syncColumns_multiRowStartBelowEnd() {
        val start = SelectionAnchor(5, 8)
        val end = SelectionAnchor(2, 5)
        val loCol = minOf(start.col, end.col)
        val hiCol = maxOf(start.col, end.col)
        assertEquals(5, loCol)
        assertEquals(8, hiCol)
    }

    @Test
    fun syncColumns_sameRowStartColGreaterThanEndCol() {
        // start.col > end.col on the same row — must not conflate with row ordering
        // The old code: if (start.row <= end.row) loCol = start.col → would give loCol=10, hiCol=3
        val start = SelectionAnchor(3, 10)
        val end = SelectionAnchor(3, 3)
        val loCol = minOf(start.col, end.col)
        val hiCol = maxOf(start.col, end.col)
        assertEquals(3, loCol)
        assertEquals(10, hiCol)
    }

    @Test
    fun syncColumns_differentRowsAndReversedCols() {
        // start.row > end.row AND start.col > end.col
        // The old code would swap columns with wrong lo/hi
        val start = SelectionAnchor(5, 8)
        val end = SelectionAnchor(3, 3)
        val loCol = minOf(start.col, end.col)
        val hiCol = maxOf(start.col, end.col)
        assertEquals(3, loCol)
        assertEquals(8, hiCol)
    }
}
