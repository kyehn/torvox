package io.torvox.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * K7 — Search includes scrollback, not just the visible screen.
 *
 * `TerminalScreen.performSearch` builds its searchable text by concatenating
 * scrollback lines (one per line, separated by '\n') followed by the visible
 * `getTerminalText()`. `findMatches` (the matcher it calls) is exercised here
 * against that same buffer shape: a term present *only* in scrollback must be
 * found, and its `lineIndex` must land inside the scrollback region
 * (index < scrollbackCount), while a term present only on the visible screen
 * must be found at an index >= scrollbackCount.
 *
 * Note: `performSearch` is a Composable-local `suspend fun` that owns the
 * bridge/surface callbacks, so it is not directly reachable from a unit test;
 * this test locks the documented scrollback-inclusive buffer contract that it
 * builds (see S4-implementation.md K7).
 */
class SearchScrollbackTest {
    private fun buildSearchableText(
        scrollbackLines: List<String>,
        visibleText: String,
    ): String {
        val builder = StringBuilder()
        for (line in scrollbackLines) {
            builder.append(line)
            builder.append('\n')
        }
        builder.append(visibleText)
        return builder.toString()
    }

    @Test
    fun findsTermPresentOnlyInScrollback() {
        val scrollback = listOf("first scrollback line", "second scrollback line")
        val visible = "visible prompt $ "
        val text = buildSearchableText(scrollback, visible)

        val results = findMatches(text, "scrollback")
        assertEquals(2, results.size)
        // Both matches live in scrollback, so their line indices are < scrollback size.
        assertTrue(results.all { it.lineIndex < scrollback.size })
        assertEquals(0, results[0].lineIndex)
        assertEquals(1, results[1].lineIndex)
    }

    @Test
    fun findsTermPresentOnlyInVisibleScreen() {
        val scrollback = listOf("scrollback only content")
        val visible = "visible prompt $ "
        val text = buildSearchableText(scrollback, visible)

        val results = findMatches(text, "visible")
        assertEquals(1, results.size)
        // Visible text is appended after all scrollback lines, so its line index
        // equals the scrollback line count.
        assertEquals(scrollback.size, results[0].lineIndex)
    }

    @Test
    fun doesNotFindTermAbsentFromEitherRegion() {
        val text = buildSearchableText(listOf("old output"), "current prompt")
        assertTrue(findMatches(text, "neverwritten").isEmpty())
    }

    @Test
    fun emptyQueryReturnsNoMatches() {
        val text = buildSearchableText(listOf("a"), "b")
        assertTrue(findMatches(text, "").isEmpty())
    }

    @Test
    fun scrollbackMatchLineIndexIsBeforeVisibleRegion() {
        val scrollbackCount = 3
        val scrollback = List(scrollbackCount) { index -> "sb$index uniqueToken" }
        val visible = "vis uniqueToken"
        val text = buildSearchableText(scrollback, visible)

        val scrollbackMatches = findMatches(text, "sb1 uniqueToken")
        assertEquals(1, scrollbackMatches.size)
        assertEquals(1, scrollbackMatches[0].lineIndex)

        val visibleMatches = findMatches(text, "vis uniqueToken")
        assertEquals(1, visibleMatches.size)
        assertEquals(scrollbackCount, visibleMatches[0].lineIndex)
    }
}
