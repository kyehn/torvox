package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * switchSession decision logic (TerminalRuntime):
 * pure guards extracted from the three-phase switch so the concurrency and
 * failure-restore branches are testable without threads or GPU state.
 */
@RunWith(RobolectricTestRunner::class)
class SwitchSessionTest {
    @Test
    fun concurrentStop_active_unchanged_returns_null() {
        assertNull(
            "same active as before the first frame → nothing to stop",
            concurrentRenderThreadToStop(
                activeSessionIdAfterRender = 2L,
                previousActiveId = 2L,
                targetId = 3L,
                concurrentSessionId = 2L,
            ),
        )
    }

    @Test
    fun concurrentStop_active_changed_returns_concurrent_session() {
        assertEquals(
            "another session published while rendering → stop its render thread",
            2L,
            concurrentRenderThreadToStop(
                activeSessionIdAfterRender = 2L,
                previousActiveId = 1L,
                targetId = 3L,
                concurrentSessionId = 2L,
            ),
        )
    }

    @Test
    fun concurrentStop_active_changed_but_session_gone_returns_null() {
        assertNull(
            "concurrent session was closed during the first frame → nothing to stop",
            concurrentRenderThreadToStop(
                activeSessionIdAfterRender = 2L,
                previousActiveId = 1L,
                targetId = 3L,
                concurrentSessionId = null,
            ),
        )
    }

    @Test
    fun concurrentStop_active_changed_to_target_returns_null() {
        assertNull(
            "active already moved to the target → no separate stop needed",
            concurrentRenderThreadToStop(
                activeSessionIdAfterRender = 3L,
                previousActiveId = 1L,
                targetId = 3L,
                concurrentSessionId = 3L,
            ),
        )
    }

    @Test
    fun restorePrevious_different_session_restores() {
        assertTrue("previous session exists and differs from the failed target", shouldRestorePreviousSession(previousId = 1L, failedTargetId = 2L))
        assertTrue("previous may be the null-active sentinel", shouldRestorePreviousSession(previousId = 0L, failedTargetId = 2L))
    }

    @Test
    fun restorePrevious_no_previous_or_same_target_skips() {
        assertFalse("no previous session → nothing to restore", shouldRestorePreviousSession(previousId = null, failedTargetId = 2L))
        assertFalse("previous == failed target → do not restore it", shouldRestorePreviousSession(previousId = 2L, failedTargetId = 2L))
    }
}
