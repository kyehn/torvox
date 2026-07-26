package io.term.ui

import io.term.SessionInfo
import io.term.TerminalState
import io.term.shouldCreateDefaultSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionManagerComprehensiveTest {
    @Test
    fun `shouldCreateDefaultSession returns true when all conditions met`() {
        assertTrue(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession returns false when surface invalid`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = false,
                surfaceWidth = 480,
                surfaceHeight = 720,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession returns false when surface width zero`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = 0,
                surfaceHeight = 720,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession returns false when surface height zero`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 0,
                uiSessions = emptyList(),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession returns false when ui sessions exist`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
                uiSessions = listOf(SessionInfo(id = 1L, title = "session1")),
                runtimeSessionIds = emptyList(),
            ),
        )
    }

    @Test
    fun `shouldCreateDefaultSession returns false when runtime sessions exist`() {
        assertFalse(
            shouldCreateDefaultSession(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
                uiSessions = emptyList(),
                runtimeSessionIds = listOf(1L),
            ),
        )
    }

    @Test
    fun `TerminalState initial values are correct`() {
        val state = TerminalState()
        assertEquals(0L, state.sessionId)
        assertFalse(state.isRunning)
        assertEquals("Terminal", state.title)
        assertEquals(io.term.ui.ModifierState.Off, state.ctrlState)
        assertEquals(io.term.ui.ModifierState.Off, state.altState)
        assertFalse(state.scrollActive)
        assertTrue(state.sessions.isEmpty())
        assertEquals(0L, state.activeSessionId)
    }

    @Test
    fun `TerminalState sessionId and activeSessionId start at 0`() {
        val state = TerminalState()
        assertEquals(0L, state.sessionId)
        assertEquals(0L, state.activeSessionId)
    }

    @Test
    fun `TerminalState ctrlState defaults to Off`() {
        assertEquals(io.term.ui.ModifierState.Off, TerminalState().ctrlState)
    }

    @Test
    fun `TerminalState scrollActive defaults to false`() {
        assertFalse(TerminalState().scrollActive)
    }

    @Test
    fun `TerminalState copy preserves unchanged fields`() {
        val state = TerminalState(ctrlState = io.term.ui.ModifierState.Locked)
        val copied = state.copy(altState = io.term.ui.ModifierState.Once)
        assertEquals(io.term.ui.ModifierState.Locked, copied.ctrlState)
        assertEquals(io.term.ui.ModifierState.Once, copied.altState)
    }

    @Test
    fun `TerminalState sessions is empty by default`() {
        assertTrue(TerminalState().sessions.isEmpty())
    }

    @Test
    fun `TerminalState sessions with content`() {
        val sessions = listOf(SessionInfo(id = 1L, title = "test"))
        val state = TerminalState(sessions = sessions, activeSessionId = 1L)
        assertEquals(1, state.sessions.size)
        assertEquals(1L, state.activeSessionId)
    }
}
