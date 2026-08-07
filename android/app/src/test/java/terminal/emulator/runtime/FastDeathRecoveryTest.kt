package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Round-224 fast-death recovery (warp WarpTerminalService.kt:906-915):
 * pure decision/backoff logic extracted from handleSessionExit.
 * Same-package as the top-level constants/functions — no imports needed.
 */
@RunWith(RobolectricTestRunner::class)
class FastDeathRecoveryTest {
    @Test
    fun fastDeath_detected_within_threshold_without_user_input() {
        assertTrue(
            "exit at 500ms with no input must retry",
            shouldRetryFastDeath(aliveMs = 500, userTypedSinceSpawn = false, fastDeathCount = 0),
        )
        assertTrue(
            "exit exactly at threshold must retry",
            shouldRetryFastDeath(aliveMs = FAST_DEATH_THRESHOLD_MS, userTypedSinceSpawn = false, fastDeathCount = 0),
        )
    }

    @Test
    fun fastDeath_not_detected_after_threshold() {
        assertFalse(
            "exit at threshold+1ms must NOT retry",
            shouldRetryFastDeath(aliveMs = FAST_DEATH_THRESHOLD_MS + 1, userTypedSinceSpawn = false, fastDeathCount = 0),
        )
        assertFalse(
            "exit at 10s must NOT retry",
            shouldRetryFastDeath(aliveMs = 10_000, userTypedSinceSpawn = false, fastDeathCount = 0),
        )
    }

    @Test
    fun fastDeath_cancelled_by_user_input() {
        assertFalse(
            "user typed → legitimate quick exit, no retry",
            shouldRetryFastDeath(aliveMs = 100, userTypedSinceSpawn = true, fastDeathCount = 0),
        )
    }

    @Test
    fun fastDeath_budget_is_bounded() {
        assertTrue(
            "count 0 (first death) may retry",
            shouldRetryFastDeath(aliveMs = 100, userTypedSinceSpawn = false, fastDeathCount = 0),
        )
        assertTrue(
            "count 2 (third death) may retry",
            shouldRetryFastDeath(aliveMs = 100, userTypedSinceSpawn = false, fastDeathCount = 2),
        )
        assertFalse(
            "count 3 (budget exhausted) must NOT retry",
            shouldRetryFastDeath(aliveMs = 100, userTypedSinceSpawn = false, fastDeathCount = 3),
        )
        assertFalse(
            "count 10 must NOT retry",
            shouldRetryFastDeath(aliveMs = 100, userTypedSinceSpawn = false, fastDeathCount = 10),
        )
    }

    @Test
    fun backoff_doubles_then_caps_at_5s() {
        assertEquals("attempt 1 → 500ms", 500L, fastDeathBackoffMs(1))
        assertEquals("attempt 2 → 1000ms", 1000L, fastDeathBackoffMs(2))
        assertEquals("attempt 3 → 2000ms", 2000L, fastDeathBackoffMs(3))
        assertEquals("attempt 4 → 4000ms", 4000L, fastDeathBackoffMs(4))
        assertEquals("attempt 5 → capped at 5000ms", 5000L, fastDeathBackoffMs(5))
        assertEquals("attempt 10 → capped at 5000ms", 5000L, fastDeathBackoffMs(10))
    }
}
