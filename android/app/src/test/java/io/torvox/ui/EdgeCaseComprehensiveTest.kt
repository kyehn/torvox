package io.torvox.ui

import io.torvox.SessionInfo
import io.torvox.TerminalState
import io.torvox.shouldCreateDefaultSession
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class EdgeCaseComprehensiveTest {
    @Test
    fun `TerminalState empty sessions list`() {
        val state = TerminalState()
        assertTrue(state.sessions.isEmpty())
    }

    @Test
    fun `TerminalState many sessions`() {
        val sessions = (1L..100L).map { SessionInfo(it, "Session $it") }
        val state = TerminalState(sessions = sessions, activeSessionId = 50L)
        assertEquals(100, state.sessions.size)
        assertEquals(50L, state.activeSessionId)
    }

    @Test
    fun `TerminalState duplicate session ids allowed in data class`() {
        val sessions =
            listOf(
                SessionInfo(1L, "first"),
                SessionInfo(1L, "second"),
            )
        val state = TerminalState(sessions = sessions)
        assertEquals(2, state.sessions.size)
    }

    @Test
    fun `TerminalState copy with empty sessions`() {
        val state = TerminalState(sessions = listOf(SessionInfo(1L, "test")))
        val copy = state.copy(sessions = emptyList())
        assertTrue(copy.sessions.isEmpty())
        assertEquals(1, state.sessions.size)
    }

    @Test
    fun `shouldCreateDefaultSession with extreme dimensions`() {
        assertTrue(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = Int.MAX_VALUE,
                surfaceHeight = Int.MAX_VALUE,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession with negative dimensions`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = -1,
                surfaceHeight = -1,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession with one dimension valid one invalid`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = 100,
                surfaceHeight = 0,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `TerminalState data class equals works correctly`() {
        val terminalStateHello = TerminalState(title = "Hello")
        val terminalStateHelloCopy = TerminalState(title = "Hello")
        val terminalStateWorld = TerminalState(title = "World")
        assertEquals(terminalStateHello, terminalStateHelloCopy)
        assertFalse(terminalStateHello == terminalStateWorld)
    }

    @Test
    fun `TerminalState hashCode is consistent with equals`() {
        val terminalStateTest = TerminalState(title = "Test")
        val terminalStateTestCopy = TerminalState(title = "Test")
        assertEquals(terminalStateTest.hashCode(), terminalStateTestCopy.hashCode())
    }

    @Test
    fun `TerminalState toString contains key fields`() {
        val state = TerminalState(title = "MyApp")
        val str = state.toString()
        assertTrue(str.contains("MyApp"))
        assertTrue(str.contains("sessionId"))
    }

    @Test
    fun `TerminalState pendingInput null by default`() {
        val state = TerminalState()
        assertNull(state.pendingInput)
    }

    @Test
    fun `TerminalState pendingInput with byte array`() {
        val input = byteArrayOf(0x1B, 0x5B, 0x41)
        val state = TerminalState(pendingInput = input)
        assertArrayEquals(input, state.pendingInput)
    }

    @Test
    fun `TerminalState selection default`() {
        val state = TerminalState()
        assertNull(state.selection.start)
        assertNull(state.selection.end)
        assertEquals("", state.selection.selectedText)
    }

    @Test
    fun `TerminalState title max length`() {
        val longTitle = "A".repeat(1000)
        val state = TerminalState(title = longTitle)
        assertEquals(1000, state.title.length)
    }

    @Test
    fun `TerminalState sessionId zero is valid`() {
        val state = TerminalState(sessionId = 0L)
        assertEquals(0L, state.sessionId)
    }

    @Test
    fun `TerminalState activeSessionId zero is valid`() {
        val state = TerminalState(activeSessionId = 0L)
        assertEquals(0L, state.activeSessionId)
    }

    @Test
    fun `TerminalState all modifiers active`() {
        val state =
            TerminalState(ctrlState = io.torvox.ui.ModifierState.Locked, altState = io.torvox.ui.ModifierState.Locked, scrollActive = true)
        assertEquals(io.torvox.ui.ModifierState.Locked, state.ctrlState)
        assertEquals(io.torvox.ui.ModifierState.Locked, state.altState)
        assertTrue(state.scrollActive)
    }
}
