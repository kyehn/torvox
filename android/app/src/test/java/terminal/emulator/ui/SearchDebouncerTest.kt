package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Host-side tests for [SearchDebouncer]: rapid typing must
 * collapse to a single search, spaced input must search per keystroke,
 * and the IME flush must bypass the debounce.
 */
class SearchDebouncerTest {
    /** Manual-clock scheduler: nothing runs until [advance] is called. */
    private class FakeScheduler : DebounceScheduler {
        var now = 0L
        private val scheduled = mutableListOf<Pair<Long, () -> Unit>>()

        override fun postDelayed(delayMillis: Long, action: () -> Unit) {
            scheduled += (now + delayMillis) to action
        }

        override fun cancelPending() {
            scheduled.clear()
        }

        /** Advance the clock; runs every action due at or before the new time. */
        fun advance(millis: Long) {
            now += millis
            val due = scheduled.filter { it.first <= now }
            scheduled.removeAll(due.toSet())
            due.forEach { (_, action) -> action() }
        }
    }

    private val debounceMillis = 150L

    @Test
    fun `rapid successive submits run only the last action once`() {
        val scheduler = FakeScheduler()
        val debouncer = SearchDebouncer(debounceMillis, scheduler)
        val executed = mutableListOf<String>()

        debouncer.submit { executed += "a" }
        debouncer.submit { executed += "b" }
        debouncer.submit { executed += "c" }
        // Nothing has run before the quiet period elapses.
        scheduler.advance(100)
        assertEquals(emptyList<String>(), executed)

        scheduler.advance(50)
        assertEquals(listOf("c"), executed)
    }

    @Test
    fun `submits spaced beyond the debounce run once each`() {
        val scheduler = FakeScheduler()
        val debouncer = SearchDebouncer(debounceMillis, scheduler)
        val executed = mutableListOf<String>()

        debouncer.submit { executed += "a" }
        scheduler.advance(150)
        debouncer.submit { executed += "b" }
        scheduler.advance(150)
        debouncer.submit { executed += "c" }
        scheduler.advance(150)

        assertEquals(listOf("a", "b", "c"), executed)
    }

    @Test
    fun `flush runs the pending action immediately and cancels the timer`() {
        val scheduler = FakeScheduler()
        val debouncer = SearchDebouncer(debounceMillis, scheduler)
        var executed = 0

        debouncer.submit { executed++ }
        assertTrue(debouncer.flush())
        assertEquals(1, executed)

        // Advancing past the debounce must not run the cancelled timer again.
        scheduler.advance(1000)
        assertEquals(1, executed)
    }

    @Test
    fun `flush with nothing pending returns false`() {
        val scheduler = FakeScheduler()
        val debouncer = SearchDebouncer(debounceMillis, scheduler)
        assertFalse(debouncer.flush())
    }

    @Test
    fun `flush after a later submit runs the latest query only`() {
        val scheduler = FakeScheduler()
        val debouncer = SearchDebouncer(debounceMillis, scheduler)
        val executed = mutableListOf<String>()

        debouncer.submit { executed += "stale" }
        debouncer.submit { executed += "latest" }
        assertTrue(debouncer.flush())
        assertEquals(listOf("latest"), executed)
        scheduler.advance(1000)
        assertEquals(listOf("latest"), executed)
    }

    @Test
    fun `cancel drops the pending action`() {
        val scheduler = FakeScheduler()
        val debouncer = SearchDebouncer(debounceMillis, scheduler)
        var executed = 0

        debouncer.submit { executed++ }
        debouncer.cancel()
        scheduler.advance(1000)
        assertEquals(0, executed)
    }
}
