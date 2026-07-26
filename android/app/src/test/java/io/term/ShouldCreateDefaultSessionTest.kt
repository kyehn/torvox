package io.term

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShouldCreateDefaultSessionTest {
    private fun allowed(
        uiSessions: List<SessionInfo> = emptyList(),
        runtimeSessionIds: List<Long> = emptyList(),
        surfaceValid: Boolean = true,
        surfaceWidth: Int = 1080,
        surfaceHeight: Int = 1920,
    ) = shouldCreateDefaultSession(
        surfaceValid = surfaceValid,
        surfaceWidth = surfaceWidth,
        surfaceHeight = surfaceHeight,
        uiSessions = uiSessions,
        runtimeSessionIds = runtimeSessionIds,
    )

    @Test
    fun allClear_returnsTrue() {
        assertTrue(allowed())
    }

    @Test
    fun invalidSurface_returnsFalse() {
        assertFalse(allowed(surfaceValid = false))
    }

    @Test
    fun zeroWidth_returnsFalse() {
        assertFalse(allowed(surfaceWidth = 0))
    }

    @Test
    fun zeroHeight_returnsFalse() {
        assertFalse(allowed(surfaceHeight = 0))
    }

    @Test
    fun negativeWidth_returnsFalse() {
        assertFalse(allowed(surfaceWidth = -1))
    }

    @Test
    fun existingUiSession_returnsFalse() {
        assertFalse(allowed(uiSessions = listOf(SessionInfo(id = 1L, title = "1"))))
    }

    @Test
    fun existingRuntimeSession_returnsFalse() {
        assertFalse(allowed(runtimeSessionIds = listOf(1L)))
    }

    @Test
    fun nonEmptyBoth_returnsFalse() {
        assertFalse(allowed(uiSessions = listOf(SessionInfo(id = 1L, title = "1")), runtimeSessionIds = listOf(1L)))
    }
}
