package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Host-side tests for the accessibility line reading logic (round-231):
 * visible-line extraction, line-by-line navigation and the 500ms
 * contentDescription throttle.
 */
class TerminalAccessibilityTest {
    /** Fake grid: rows outside the map are blank (null), like scrollbackLine. */
    private class FakeLineSource(
        private val grid: Map<Int, String>,
    ) : AccessibilityLineSource {
        override fun line(row: Int): String? = grid[row]
    }

    // ── AccessibilityLineProvider ─────────────────────────────────────

    @Test
    fun `visible lines are trimmed and blank rows dropped`() {
        val provider =
            AccessibilityLineProvider(
                FakeLineSource(
                    mapOf(
                        2 to "  prompt$ ",
                        3 to "   ",
                        4 to "output",
                    ),
                ),
            )
        val lines = provider.visibleLines(rows = 4, scrollbackLength = 5, scrollOffset = 3)
        assertEquals(
            listOf(AccessibilityLine(2, "  prompt$"), AccessibilityLine(4, "output")),
            lines,
        )
    }

    @Test
    fun `empty screen yields no lines`() {
        val provider = AccessibilityLineProvider(FakeLineSource(emptyMap()))
        assertEquals(emptyList<AccessibilityLine>(), provider.visibleLines(4, 5, 1))
    }

    @Test
    fun `visible lines still expose the viewport when scrollback is empty`() {
        // Round-232 regression: the surface used to bail out when
        // scrollbackLength == 0, so TalkBack never read the visible screen
        // before the first scrollback line piled up. The provider must map
        // rows 0..rows-1 to the grid even with zero scrollback.
        val provider =
            AccessibilityLineProvider(
                FakeLineSource(
                    mapOf(
                        0 to "prompt$",
                        1 to "output",
                    ),
                ),
            )
        assertEquals(
            listOf(AccessibilityLine(0, "prompt$"), AccessibilityLine(1, "output")),
            provider.visibleLines(rows = 3, scrollbackLength = 0, scrollOffset = 0),
        )
    }

    @Test
    fun `rows clamp below zero when scrolled past the top`() {
        val provider =
            AccessibilityLineProvider(
                FakeLineSource(mapOf(0 to "first", 1 to "second")),
            )
        // scrollOffset beyond scrollbackLength would make firstRow negative.
        val lines = provider.visibleLines(rows = 2, scrollbackLength = 1, scrollOffset = 5)
        assertEquals(listOf(AccessibilityLine(0, "first"), AccessibilityLine(1, "second")), lines)
    }

    @Test
    fun `content description joins lines and caps length`() {
        val provider = AccessibilityLineProvider(FakeLineSource(emptyMap()))
        val short = provider.contentDescription(listOf(AccessibilityLine(0, "a"), AccessibilityLine(1, "b")))
        assertEquals("a\nb", short)

        val longText = "x".repeat(AccessibilityLineProvider.MAX_DESCRIPTION_CHARS + 50)
        val capped = provider.contentDescription(listOf(AccessibilityLine(0, longText)))
        assertEquals(AccessibilityLineProvider.MAX_DESCRIPTION_CHARS + 1, capped.length)
        assertEquals("…", capped.last().toString())
    }

    // ── AccessibilityLineNavigator ────────────────────────────────────

    private fun navigatorWith(lines: Map<Int, String>): AccessibilityLineNavigator = AccessibilityLineNavigator(AccessibilityLineProvider(FakeLineSource(lines)))

    @Test
    fun `next walks down and wraps to the first line`() {
        val navigator =
            navigatorWith(
                mapOf(0 to "alpha", 1 to "beta", 2 to "gamma"),
            )
        assertEquals(AccessibilityLine(0, "alpha"), navigator.current(3, 0, 0))
        assertEquals(AccessibilityLine(1, "beta"), navigator.next(3, 0, 0))
        assertEquals(AccessibilityLine(2, "gamma"), navigator.next(3, 0, 0))
        assertEquals(AccessibilityLine(0, "alpha"), navigator.next(3, 0, 0))
    }

    @Test
    fun `previous walks up and wraps to the last line`() {
        val navigator =
            navigatorWith(
                mapOf(0 to "alpha", 1 to "beta", 2 to "gamma"),
            )
        assertEquals(AccessibilityLine(0, "alpha"), navigator.current(3, 0, 0))
        assertEquals(AccessibilityLine(2, "gamma"), navigator.previous(3, 0, 0))
        assertEquals(AccessibilityLine(1, "beta"), navigator.previous(3, 0, 0))
        assertEquals(AccessibilityLine(0, "alpha"), navigator.previous(3, 0, 0))
    }

    @Test
    fun `navigation on an empty screen returns null`() {
        val navigator = navigatorWith(emptyMap())
        assertNull(navigator.current(3, 3, 0))
        assertNull(navigator.next(3, 3, 0))
        assertNull(navigator.previous(3, 3, 0))
    }

    @Test
    fun `current keeps the same grid row across a scroll`() {
        val navigator =
            navigatorWith(
                mapOf(1 to "one", 2 to "two", 3 to "three", 4 to "four"),
            )
        // Viewport showing rows 1..2; read row 1 then "next" to row 2.
        navigator.current(2, 4, 3)
        navigator.next(2, 4, 3)
        assertEquals(AccessibilityLine(2, "two"), navigator.current(2, 4, 3))
        // Scroll toward newer content: viewport now shows rows 2..3 and
        // row 2 is still visible, so the current line is preserved.
        assertEquals(AccessibilityLine(2, "two"), navigator.current(2, 4, 2))
        // Scroll further: row 2 leaves the viewport (rows 3..4); current
        // falls back to the top visible line.
        assertEquals(AccessibilityLine(3, "three"), navigator.current(2, 4, 1))
    }

    // ── DebouncedTextUpdater ──────────────────────────────────────────

    private class FakeScheduler : DebounceScheduler {
        var now = 0L
        private val scheduled = mutableListOf<Pair<Long, () -> Unit>>()

        override fun postDelayed(delayMillis: Long, action: () -> Unit) {
            scheduled += (now + delayMillis) to action
        }

        override fun cancelPending() {
            scheduled.clear()
        }

        fun advance(millis: Long) {
            now += millis
            val due = scheduled.filter { it.first <= now }
            scheduled.removeAll(due.toSet())
            due.forEach { (_, action) -> action() }
        }
    }

    @Test
    fun `rapid updates emit only the latest text after the quiet period`() {
        val scheduler = FakeScheduler()
        val updater = DebouncedTextUpdater(500L, scheduler)
        val emitted = mutableListOf<String>()

        updater.update("screen A", emitted::add)
        updater.update("screen B", emitted::add)
        updater.update("screen C", emitted::add)
        scheduler.advance(499)
        assertEquals(emptyList<String>(), emitted)
        scheduler.advance(1)
        assertEquals(listOf("screen C"), emitted)
        // The cancelled timers must not fire later.
        scheduler.advance(10_000)
        assertEquals(listOf("screen C"), emitted)
    }

    @Test
    fun `identical text is not re-scheduled`() {
        val scheduler = FakeScheduler()
        val updater = DebouncedTextUpdater(500L, scheduler)
        val emitted = mutableListOf<String>()

        updater.update("same", emitted::add)
        updater.update("same", emitted::add)
        scheduler.advance(500)
        assertEquals(listOf("same"), emitted)
    }

    @Test
    fun `cancel drops the pending text`() {
        val scheduler = FakeScheduler()
        val updater = DebouncedTextUpdater(500L, scheduler)
        val emitted = mutableListOf<String>()

        updater.update("dropped", emitted::add)
        updater.cancel()
        scheduler.advance(500)
        assertEquals(emptyList<String>(), emitted)
    }
}
