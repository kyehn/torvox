package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [TerminalRuntime.maybeRequestSurfaceRecreate] 背后的纯限流：首帧失效立即请求换新窗口，
 * 快试阶段按最小间隔重试，用尽后转为慢重试——既不无限拆装视图，也不永久放弃自愈。
 */
class DecideSurfaceRecreateTest {

    private val minIntervalNanos = 500_000_000L
    private val slowIntervalNanos = 5_000_000_000L

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
    fun `slow phase reports exhaustion and waits for its own interval`() {
        val decision = decideSurfaceRecreate(
            attempts = 5,
            lastRequestNanos = 1_000_000_000L,
            nowNanos = 1_000_000_000L + slowIntervalNanos - 1,
        )
        assertFalse(decision.request)
        assertTrue(decision.exhausted)
    }

    @Test
    fun `slow phase keeps retrying after its interval`() {
        val decision = decideSurfaceRecreate(
            attempts = 5,
            lastRequestNanos = 1_000_000_000L,
            nowNanos = 1_000_000_000L + slowIntervalNanos,
        )
        assertTrue(decision.request)
        assertTrue(decision.exhausted)
    }

    @Test
    fun `slow phase does not retry at the fast cadence`() {
        val decision = decideSurfaceRecreate(
            attempts = 99,
            lastRequestNanos = 1L,
            nowNanos = 1L + minIntervalNanos * 2,
        )
        assertFalse(decision.request)
        assertTrue(decision.exhausted)
    }
}
