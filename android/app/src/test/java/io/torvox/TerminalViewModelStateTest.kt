package io.torvox

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalViewModelStateTest {
    @Test
    fun startSelectionCreatesActiveAnchor() {
        val state = TerminalState()
        val sut = TerminalViewModelDelegate(state)
        val result = sut.startSelection(row = 5, col = 3)
        assertTrue(result.selection.active)
        assertNotNull(result.selection.start)
        assertEquals(5, result.selection.start!!.row)
        assertEquals(3, result.selection.start!!.col)
        assertEquals(result.selection.start, result.selection.end)
    }

    @Test
    fun updateSelectionExtendsEndAnchor() {
        val state = TerminalState()
        val sut = TerminalViewModelDelegate(state)
        val afterStart = sut.startSelection(row = 2, col = 1)
        val afterUpdate = sut.updateSelection(4, 7, afterStart)
        assertTrue(afterUpdate.selection.active)
        assertEquals(2, afterUpdate.selection.start!!.row)
        assertEquals(1, afterUpdate.selection.start!!.col)
        assertEquals(4, afterUpdate.selection.end!!.row)
        assertEquals(7, afterUpdate.selection.end!!.col)
    }

    @Test
    fun updateSelectionWithoutActiveDoesNothing() {
        val state = TerminalState()
        val sut = TerminalViewModelDelegate(state)
        val result = sut.updateSelection(1, 1, state)
        assertFalse(result.selection.active)
        assertNull(result.selection.start)
    }

    @Test
    fun endSelectionWithoutActiveDoesNothing() {
        val state = TerminalState()
        val sut = TerminalViewModelDelegate(state)
        val result = sut.endSelection(state)
        assertEquals("", result.selection.selectedText)
        assertNull(result.selection.start)
    }

    @Test
    fun setSelectionModeUpdatesMode() {
        val state = TerminalState()
        val sut = TerminalViewModelDelegate(state)
        val result = sut.setSelectionMode(SelectionMode.Word, state)
        assertEquals(SelectionMode.Word, result.selection.mode)
    }

    @Test
    fun setSelectionModeCharIsDefault() {
        val state = TerminalState()
        assertEquals(SelectionMode.Char, state.selection.mode)
    }

    @Test
    fun clearSelectionResetsToDefault() {
        val state = TerminalState(selection = SelectionState(active = true, selectedText = "hello"))
        val sut = TerminalViewModelDelegate(state)
        val result = sut.clearSelection()
        assertFalse(result.selection.active)
        assertNull(result.selection.start)
        assertNull(result.selection.end)
        assertEquals("", result.selection.selectedText)
    }

    @Test
    fun toggleScrollModeFlipsState() {
        val state = TerminalState()
        val sut = TerminalViewModelDelegate(state)
        val toggledOn = sut.toggleScrollMode(state)
        assertTrue(toggledOn.scrollActive)
        val toggledOff = sut.toggleScrollMode(toggledOn)
        assertFalse(toggledOff.scrollActive)
    }

    @Test
    fun modifierKeysDefaultIsPopulated() {
        val state = TerminalState()
        assertTrue(state.modifierKeys.isNotEmpty())
    }

    @Test
    fun resetModifierKeysRestoresDefault() {
        val state = TerminalState(modifierKeys = emptyList())
        val sut = TerminalViewModelDelegate(state)
        val result = sut.resetModifierKeys()
        assertTrue(result.modifierKeys.isNotEmpty())
    }

    @Test
    fun closeSessionWithOneSessionClearsAll() {
        val sessions = listOf(SessionInfo(id = 1L, title = "Session 1"))
        val state = TerminalState(sessions = sessions, activeSessionId = 1L, sessionId = 1L, isRunning = true)
        val sut = TerminalViewModelDelegate(state)
        val result = sut.closeSession(1L, state)
        assertFalse(result.isRunning)
        assertTrue(result.sessions.isEmpty())
        assertEquals(0L, result.activeSessionId)
    }

    @Test
    fun closeSessionWithMultipleSessionsSelectsLast() {
        val sessions =
            listOf(
                SessionInfo(id = 1L, title = "Session 1"),
                SessionInfo(id = 2L, title = "Session 2"),
            )
        val state = TerminalState(sessions = sessions, activeSessionId = 1L, sessionId = 1L, isRunning = true)
        val sut = TerminalViewModelDelegate(state)
        val result = sut.closeSession(1L, state)
        assertEquals(1, result.sessions.size)
        assertEquals(2L, result.activeSessionId)
        assertEquals(2L, result.sessionId)
    }

    @Test
    fun closeSessionOfNonActivePreservesActive() {
        val sessions =
            listOf(
                SessionInfo(id = 1L, title = "Session 1"),
                SessionInfo(id = 2L, title = "Session 2"),
            )
        val state = TerminalState(sessions = sessions, activeSessionId = 2L, sessionId = 2L, isRunning = true)
        val sut = TerminalViewModelDelegate(state)
        val result = sut.closeSession(1L, state)
        assertEquals(1, result.sessions.size)
        assertEquals(2L, result.activeSessionId)
    }

    @Test
    fun startSelectionUsesExistingMode() {
        val state = TerminalState(selection = SelectionState(mode = SelectionMode.Line))
        val sut = TerminalViewModelDelegate(state)
        val result = sut.startSelection(row = 0, col = 0, currentState = state)
        assertEquals(SelectionMode.Line, result.selection.mode)
    }

    @Test
    fun endSelectionWithoutStartDoesNothing() {
        val state = TerminalState(selection = SelectionState(active = true, start = null, end = null))
        val sut = TerminalViewModelDelegate(state)
        val result = sut.endSelection(state)
        assertEquals(state, result)
    }

    @Test
    fun endSelectionWithoutEndDoesNothing() {
        val state =
            TerminalState(
                selection = SelectionState(active = true, start = SelectionAnchor(0, 0), end = null),
            )
        val sut = TerminalViewModelDelegate(state)
        val result = sut.endSelection(state)
        assertEquals(state, result)
    }
}

internal class TerminalViewModelDelegate(
    private var state: TerminalState,
) {
    fun startSelection(
        row: Int,
        col: Int,
        currentState: TerminalState = state,
    ): TerminalState {
        val anchor = SelectionAnchor(row, col)
        return currentState.copy(
            selection =
            SelectionState(
                active = true,
                start = anchor,
                end = anchor,
                mode = currentState.selection.mode,
            ),
        )
    }

    fun updateSelection(
        row: Int,
        col: Int,
        currentState: TerminalState = state,
    ): TerminalState {
        if (!currentState.selection.active) return currentState
        return currentState.copy(
            selection = currentState.selection.copy(end = SelectionAnchor(row, col)),
        )
    }

    fun endSelection(currentState: TerminalState = state): TerminalState {
        val sel = currentState.selection
        if (!sel.active || sel.start == null || sel.end == null) return currentState
        return currentState.copy(
            selection = sel.copy(active = false, selectedText = "sample text"),
        )
    }

    fun setSelectionMode(
        mode: SelectionMode,
        currentState: TerminalState = state,
    ): TerminalState = currentState.copy(
        selection = currentState.selection.copy(mode = mode),
    )

    fun clearSelection(): TerminalState = TerminalState()

    fun toggleScrollMode(currentState: TerminalState = state): TerminalState = currentState.copy(scrollActive = !currentState.scrollActive)

    fun resetModifierKeys(): TerminalState = TerminalState()

    fun closeSession(
        id: Long,
        currentState: TerminalState = state,
    ): TerminalState {
        val remaining = currentState.sessions.filter { it.id != id }
        if (remaining.isEmpty()) {
            return currentState.copy(
                isRunning = false,
                sessions = emptyList(),
                activeSessionId = 0L,
                selection = SelectionState(),
                pendingInput = null,
            )
        }
        val newActive =
            if (currentState.activeSessionId == id) {
                remaining.last().id
            } else {
                currentState.activeSessionId
            }
        val activeSession = remaining.find { it.id == newActive }
        return currentState.copy(
            sessions = remaining,
            activeSessionId = newActive,
            sessionId = newActive,
            title = activeSession?.title ?: "Torvox",
            selection = SelectionState(),
            pendingInput = null,
        )
    }

    fun createSessionWithSurface(
        surfaceValid: Boolean,
        surfaceWidth: Int,
        surfaceHeight: Int,
        currentState: TerminalState = state,
    ): TerminalState {
        if (!surfaceValid || surfaceWidth <= 0 || surfaceHeight <= 0) {
            return currentState
        }
        val newId = (currentState.sessions.maxOfOrNull { it.id } ?: 0L) + 1
        val info = SessionInfo(id = newId, title = "Session $newId")
        val sessions = (currentState.sessions + info).sortedBy { it.id }
        return currentState.copy(
            sessionId = newId,
            isRunning = true,
            title = info.title,
            selection = SelectionState(),
            pendingInput = null,
            sessions = sessions,
            activeSessionId = newId,
        )
    }

    fun switchSessionWithSurface(
        id: Long,
        surfaceValid: Boolean,
        surfaceWidth: Int,
        surfaceHeight: Int,
        currentState: TerminalState = state,
    ): TerminalState {
        if (!surfaceValid || surfaceWidth == 0 || surfaceHeight == 0) {
            return currentState
        }
        val session = currentState.sessions.find { it.id == id } ?: return currentState
        return currentState.copy(
            sessionId = id,
            isRunning = true,
            title = session.title,
            activeSessionId = id,
            selection = SelectionState(),
            pendingInput = null,
        )
    }

    fun simulateSurfaceDestroyed(currentState: TerminalState = state): TerminalState = currentState.copy(isRunning = false)

    fun simulateSurfaceAvailable(
        surfaceValid: Boolean,
        surfaceWidth: Int,
        surfaceHeight: Int,
        currentState: TerminalState = state,
    ): TerminalState {
        if (!surfaceValid || surfaceWidth <= 0 || surfaceHeight <= 0) {
            return currentState
        }
        return currentState.copy(isRunning = true)
    }
}
