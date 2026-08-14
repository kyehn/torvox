package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Fast-death recovery (warp WarpTerminalService.kt:906-915):
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

    @Test
    fun resolveAliveMs_prefers_native_measurement() {
        assertEquals("native alive_ms wins", 3_000L, resolveAliveMs(aliveMs = 3_000, nowMs = 9_000, spawnedAtMs = 1_000))
    }

    @Test
    fun resolveAliveMs_falls_back_to_wall_clock_when_native_missing() {
        // aliveMs <= 0 means the event predates the native field.
        assertEquals("zero alive_ms falls back", 8_000L, resolveAliveMs(aliveMs = 0, nowMs = 9_000, spawnedAtMs = 1_000))
        assertEquals("negative alive_ms falls back", 8_000L, resolveAliveMs(aliveMs = -1, nowMs = 9_000, spawnedAtMs = 1_000))
        // A clock anomaly must not produce a negative lifetime.
        assertEquals("fallback clamps to zero", 0L, resolveAliveMs(aliveMs = 0, nowMs = 1_000, spawnedAtMs = 9_000))
    }

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
}
