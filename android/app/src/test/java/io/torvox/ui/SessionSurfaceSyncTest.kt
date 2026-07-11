package io.torvox.ui

import io.torvox.SessionInfo
import io.torvox.TerminalState
import io.torvox.TerminalViewModelDelegate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionSurfaceSyncTest {
    @Test
    fun `session creation validates surface before proceeding`() {
        // When surface is invalid, createSession should return without changes
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = false,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertEquals(state, result)
    }

    @Test
    fun `session creation validates dimensions before proceeding`() {
        // When dimensions are zero, createSession should return without changes
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 0,
                surfaceHeight = 720,
            )
        assertEquals(state, result)
    }

    @Test
    fun `session creation succeeds with valid surface`() {
        // When surface is valid, createSession should add a new session
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertTrue(result.sessions.isNotEmpty())
        assertTrue(result.isRunning)
        assertTrue(result.activeSessionId > 0)
    }

    @Test
    fun `switching sessions validates surface`() {
        // When surface is invalid, switchSession should return without changes
        val sessions =
            listOf(
                SessionInfo(id = 1L, title = "Session 1"),
                SessionInfo(id = 2L, title = "Session 2"),
            )
        val state =
            TerminalState(
                sessions = sessions,
                activeSessionId = 1L,
                sessionId = 1L,
            )
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.switchSessionWithSurface(
                id = 2L,
                surfaceValid = false,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertEquals(1L, result.activeSessionId)
    }

    @Test
    fun `switching sessions validates dimensions`() {
        // When dimensions are zero, switchSession should return without changes
        val sessions =
            listOf(
                SessionInfo(id = 1L, title = "Session 1"),
                SessionInfo(id = 2L, title = "Session 2"),
            )
        val state =
            TerminalState(
                sessions = sessions,
                activeSessionId = 1L,
                sessionId = 1L,
            )
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.switchSessionWithSurface(
                id = 2L,
                surfaceValid = true,
                surfaceWidth = 0,
                surfaceHeight = 0,
            )
        assertEquals(1L, result.activeSessionId)
    }

    @Test
    fun `switching sessions succeeds with valid surface`() {
        // When surface is valid, switchSession should change active session
        val sessions =
            listOf(
                SessionInfo(id = 1L, title = "Session 1"),
                SessionInfo(id = 2L, title = "Session 2"),
            )
        val state =
            TerminalState(
                sessions = sessions,
                activeSessionId = 1L,
                sessionId = 1L,
            )
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.switchSessionWithSurface(
                id = 2L,
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertEquals(2L, result.activeSessionId)
        assertEquals("Session 2", result.title)
    }

    @Test
    fun `switching to non-existent session does nothing`() {
        // When session ID doesn't exist, switchSession should return without changes
        val sessions =
            listOf(
                SessionInfo(id = 1L, title = "Session 1"),
            )
        val state =
            TerminalState(
                sessions = sessions,
                activeSessionId = 1L,
                sessionId = 1L,
            )
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.switchSessionWithSurface(
                id = 99L,
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertEquals(1L, result.activeSessionId)
    }

    @Test
    fun `surface destruction marks session as not running`() {
        // When surface is destroyed, the session should be marked as not running
        val state = TerminalState(isRunning = true)
        val delegate = TerminalViewModelDelegate(state)
        val result = delegate.simulateSurfaceDestroyed()
        assertFalse(result.isRunning)
    }

    @Test
    fun `surface recreation allows session creation`() {
        // After surface recreation, session creation should work
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)

        // Simulate surface available
        val afterSurface =
            delegate.simulateSurfaceAvailable(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertTrue(afterSurface.isRunning)

        // Now create session - should succeed
        val afterSession =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertTrue(afterSession.sessions.isNotEmpty())
    }

    @Test
    fun `multiple sessions can be created with valid surface`() {
        // Verify that multiple sessions can be created sequentially
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)

        // Create first session
        val afterFirst =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertEquals(1, afterFirst.sessions.size)

        // Create second session
        val afterSecond =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
                currentState = afterFirst,
            )
        assertEquals(2, afterSecond.sessions.size)
    }
}
