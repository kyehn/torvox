package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Render-thread recovery decisions (dead-render restart budget/backoff, initial render retry):
 * pure decision logic extracted from the runtime. Fast-death shell respawn was removed
 * (启动入口失败不得回退) together with its helpers and tests.
 */
@RunWith(RobolectricTestRunner::class)
class FastDeathRecoveryTest {
    @Test
    fun deadRenderRestartDelay_doubles_up_to_max() {
        assertEquals("100 → 200", 200L, nextRestartDelayMs(100L, maxDelayMs = 1_000L))
        assertEquals("200 → 400", 400L, nextRestartDelayMs(200L, maxDelayMs = 1_000L))
        assertEquals("400 → 800", 800L, nextRestartDelayMs(400L, maxDelayMs = 1_000L))
        assertEquals("800 → capped at 1000", 1_000L, nextRestartDelayMs(800L, maxDelayMs = 1_000L))
        assertEquals("1000 stays capped", 1_000L, nextRestartDelayMs(1_000L, maxDelayMs = 1_000L))
    }

    @Test
    fun deadRenderRestartBudget_closes_after_max_attempts() {
        assertFalse("attempt 5 (== max) still restarts", shouldCloseDeadRender(restartAttempts = 5, maxAttempts = 5))
        assertTrue("attempt 6 exceeds the budget", shouldCloseDeadRender(restartAttempts = 6, maxAttempts = 5))
        assertTrue("attempt 10 exceeds the budget", shouldCloseDeadRender(restartAttempts = 10, maxAttempts = 5))
        assertFalse("attempt 0 restarts", shouldCloseDeadRender(restartAttempts = 0, maxAttempts = 5))
    }

    @Test
    fun initialRenderRetry_retries_until_success_or_budget() {
        assertTrue("failed result under budget retries", initialRenderRetryNeeded(result = -1, attempts = 1, maxAttempts = 3))
        assertTrue("mid-budget failure still retries", initialRenderRetryNeeded(result = -1, attempts = 2, maxAttempts = 3))
        assertFalse("success stops the loop", initialRenderRetryNeeded(result = 0, attempts = 1, maxAttempts = 3))
        assertFalse("positive result stops the loop", initialRenderRetryNeeded(result = 1, attempts = 1, maxAttempts = 3))
        assertFalse("attempt == max stops even on failure", initialRenderRetryNeeded(result = -1, attempts = 3, maxAttempts = 3))
    }
}
