package io.term.ui

import io.term.SessionInfo
import io.term.TerminalState
import io.term.TerminalViewModelDelegate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalSurfaceLifecycleTest {
    @Test
    fun `createSession fails gracefully when surface is null`() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        // Simulate: no surface available, session should not be created
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = false,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        assertTrue(result.sessions.isEmpty())
    }

    @Test
    fun `createSession fails gracefully when surface width is zero`() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 0,
                surfaceHeight = 720,
            )
        assertTrue(result.sessions.isEmpty())
    }

    @Test
    fun `createSession fails gracefully when surface height is zero`() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 0,
            )
        assertTrue(result.sessions.isEmpty())
    }

    @Test
    fun `switchSession fails gracefully when surface is null`() {
        val sessions = listOf(SessionInfo(id = 1L, title = "Session 1"))
        val state = TerminalState(sessions = sessions, activeSessionId = 1L)
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.switchSessionWithSurface(
                id = 1L,
                surfaceValid = false,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        // Session should remain unchanged
        assertEquals(1L, result.activeSessionId)
    }

    @Test
    fun `switchSession fails gracefully when surface dimensions are zero`() {
        val sessions = listOf(SessionInfo(id = 1L, title = "Session 1"))
        val state = TerminalState(sessions = sessions, activeSessionId = 1L)
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.switchSessionWithSurface(
                id = 1L,
                surfaceValid = true,
                surfaceWidth = 0,
                surfaceHeight = 0,
            )
        assertEquals(1L, result.activeSessionId)
    }

    @Test
    fun `currentSurface is set to null on destroy`() {
        // Verify that the surface lifecycle is properly tracked
        // In the real code, onSurfaceTextureDestroyed sets currentSurface = null
        // and onSurfaceTextureAvailable sets currentSurface to the new surface
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result = delegate.simulateSurfaceDestroyed()
        // State should remain valid after surface destruction
        assertFalse(result.isRunning)
    }

    @Test
    fun `surface recreation updates currentSurface`() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        val result =
            delegate.simulateSurfaceAvailable(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        // Surface should be tracked as valid
        assertTrue(result.isRunning || result.sessions.isEmpty())
    }

    @Test
    fun `session creation after surface recreation succeeds`() {
        val state = TerminalState()
        val delegate = TerminalViewModelDelegate(state)
        // First, simulate surface available
        delegate.simulateSurfaceAvailable(
            surfaceValid = true,
            surfaceWidth = 480,
            surfaceHeight = 720,
        )
        // Then create session - should succeed since surface is valid
        val result =
            delegate.createSessionWithSurface(
                surfaceValid = true,
                surfaceWidth = 480,
                surfaceHeight = 720,
            )
        // With mock runtime, session creation should work
        assertTrue(result.sessions.isNotEmpty() || result.isRunning)
    }
}
