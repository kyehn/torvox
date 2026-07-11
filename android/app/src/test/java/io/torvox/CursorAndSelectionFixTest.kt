package io.torvox

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class CursorAndSelectionFixTest {
    @Test
    fun partialSelectionStartToEnd() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 0, col = 0)
        val updated = delegate.updateSelection(row = 1, col = 20, started)
        val ended = delegate.endSelection(updated)
        assertFalse(ended.selection.active)
        assertEquals(0, ended.selection.start!!.row)
        assertEquals(0, ended.selection.start!!.col)
        assertEquals(1, ended.selection.end!!.row)
        assertEquals(20, ended.selection.end!!.col)
    }

    @Test
    fun partialSelectionReverseDirection() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 5, col = 30)
        val updated = delegate.updateSelection(row = 2, col = 5, started)
        val ended = delegate.endSelection(updated)
        assertFalse(ended.selection.active)
        assertEquals(5, ended.selection.start!!.row)
        assertEquals(30, ended.selection.start!!.col)
        assertEquals(2, ended.selection.end!!.row)
        assertEquals(5, ended.selection.end!!.col)
    }

    @Test
    fun singleCellSelection() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 3, col = 10)
        val ended = delegate.endSelection(started)
        assertFalse(ended.selection.active)
        assertEquals(3, ended.selection.start!!.row)
        assertEquals(10, ended.selection.start!!.col)
        assertEquals(3, ended.selection.end!!.row)
        assertEquals(10, ended.selection.end!!.col)
    }

    @Test
    fun selectionClearedOnInputResetsAll() {
        val state =
            TerminalState(
                selection =
                SelectionState(
                    active = true,
                    start = SelectionAnchor(2, 5),
                    end = SelectionAnchor(4, 15),
                    selectedText = "some selected text",
                ),
            )
        val delegate = TerminalViewModelDelegate(state)
        val cleared = delegate.clearSelection()
        assertFalse(cleared.selection.active)
        assertNull(cleared.selection.start)
        assertNull(cleared.selection.end)
        assertEquals("", cleared.selection.selectedText)
    }

    @Test
    fun selectAllCoversFullGrid() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 0, col = 0)
        val updated = delegate.updateSelection(row = 23, col = 79, started)
        val ended = delegate.endSelection(updated)
        assertEquals(0, ended.selection.start!!.row)
        assertEquals(0, ended.selection.start!!.col)
        assertEquals(23, ended.selection.end!!.row)
        assertEquals(79, ended.selection.end!!.col)
    }

    @Test
    fun selectionModeWordPreserved() {
        val state = TerminalState(selection = SelectionState(mode = SelectionMode.Word))
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 0, col = 0, currentState = state)
        assertEquals(SelectionMode.Word, started.selection.mode)
    }

    @Test
    fun selectionModeLinePreserved() {
        val state = TerminalState(selection = SelectionState(mode = SelectionMode.Line))
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 0, col = 0, currentState = state)
        assertEquals(SelectionMode.Line, started.selection.mode)
    }

    @Test
    fun selectionModeBlockPreserved() {
        val state = TerminalState(selection = SelectionState(mode = SelectionMode.Block))
        val delegate = TerminalViewModelDelegate(state)
        val started = delegate.startSelection(row = 0, col = 0, currentState = state)
        assertEquals(SelectionMode.Block, started.selection.mode)
    }
}
