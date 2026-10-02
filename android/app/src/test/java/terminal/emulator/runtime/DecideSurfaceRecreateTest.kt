package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure surface-recreate throttling behind [TerminalRuntime.maybeRequestSurfaceRecreate]:
 * the first invalid frame requests a fresh Android surface immediately, retries wait for the
 * minimum interval, and the attempt cap stops the churn once rebuilding looks hopeless.
 */
class DecideSurfaceRecreateTest {

    private val minIntervalNanos = 500_000_000L

    @Test
    fun `first failure requests immediately`() {
        val decision = decideSurfaceRecreate(attempts = 0, lastRequestNanos = 0L, nowNanos = 12345L)
        assertTrue(decision.request)
        assertFalse(decision.exhausted)
    }

    @Test
    fun `retry inside the interval is withheld`() {
        val decision = decideSurfaceRecreate(
            attempts = 1,
            lastRequestNanos = 1_000_000_000L,
            nowNanos = 1_000_000_000L + minIntervalNanos - 1,
        )
        assertFalse(decision.request)
        assertFalse(decision.exhausted)
    }

    @Test
    fun `retry after the interval is allowed`() {
        val decision = decideSurfaceRecreate(
            attempts = 1,
            lastRequestNanos = 1_000_000_000L,
            nowNanos = 1_000_000_000L + minIntervalNanos,
        )
        assertTrue(decision.request)
    }

    @Test
    fun `attempt cap exhausts instead of requesting`() {
        val decision = decideSurfaceRecreate(attempts = 5, lastRequestNanos = 0L, nowNanos = 1L)
        assertFalse(decision.request)
        assertTrue(decision.exhausted)
    }

    @Test
    fun `exhaustion wins over a long idle interval`() {
        val decision = decideSurfaceRecreate(
            attempts = 99,
            lastRequestNanos = 1L,
            nowNanos = 1L + minIntervalNanos * 100,
        )
        assertFalse(decision.request)
        assertTrue(decision.exhausted)
    }

    @Test
    fun `reset budget after recovery allows a fresh burst`() {
        // Recovery clears both the attempts and the timestamp, so the next invalidation
        // (a second abandoned BufferQueue later in the session) starts over at attempt 1.
        val recovered = decideSurfaceRecreate(attempts = 0, lastRequestNanos = 0L, nowNanos = 42L)
        assertEquals(1, if (recovered.request) 1 else 0)
    }
}
