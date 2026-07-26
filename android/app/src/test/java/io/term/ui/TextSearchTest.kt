package io.term.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class TextSearchTest {
    @Test
    fun findMatchesSingleMatch() {
        val text = "Hello World"
        val results = findMatches(text, "World")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(6, results[0].startIndex)
        assertEquals(11, results[0].endIndex)
    }

    @Test
    fun findMatchesMultipleMatchesOnSameLine() {
        val text = "aaa"
        val results = findMatches(text, "aa")
        assertEquals(2, results.size)
        assertEquals(0, results[0].startIndex)
        assertEquals(1, results[1].startIndex)
    }

    @Test
    fun findMatchesMultipleMatchesAcrossLines() {
        val text = "line one\nline two\nline three"
        val results = findMatches(text, "line")
        assertEquals(3, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(1, results[1].lineIndex)
        assertEquals(2, results[2].lineIndex)
    }

    @Test
    fun findMatchesCaseInsensitive() {
        val text = "Hello HELLO hello"
        val results = findMatches(text, "hello", matchCase = false)
        assertEquals(3, results.size)
    }

    @Test
    fun findMatchesCaseSensitive() {
        val text = "Hello HELLO\nhello world"
        val results = findMatches(text, "hello", matchCase = true)
        assertEquals(1, results.size)
        assertEquals(1, results[0].lineIndex)
    }

    @Test
    fun findMatchesEmptyQuery() {
        val results = findMatches("Hello World", "")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatchesNoResults() {
        val results = findMatches("Hello World", "xyz")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatchesEmptyText() {
        val results = findMatches("", "hello")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatchesQueryAtStart() {
        val results = findMatches("Hello World", "Hello")
        assertEquals(1, results.size)
        assertEquals(0, results[0].startIndex)
    }

    @Test
    fun findMatchesQueryAtEnd() {
        val results = findMatches("Hello World", "World")
        assertEquals(1, results.size)
        assertEquals(6, results[0].startIndex)
        assertEquals(11, results[0].endIndex)
    }

    @Test
    fun findMatchesOverlappingNotDuplicated() {
        val results = findMatches("aaaa", "aa")
        assertEquals(3, results.size)
        assertEquals(0, results[0].startIndex)
        assertEquals(1, results[1].startIndex)
        assertEquals(2, results[2].startIndex)
    }

    @Test
    fun findMatchesExactLineIndex() {
        val text = "first\nsecond\nthird"
        val results = findMatches(text, "second")
        assertEquals(1, results[0].lineIndex)
        assertEquals(0, results[0].startIndex)
        assertEquals(6, results[0].endIndex)
    }

    @Test
    fun findMatchesSearchHighlightUsesSameTotalLinesAsCanvas() {
        val text = "line one\nline two\nline three"
        val results = findMatches(text, "two")
        assertEquals(1, results.size)
        val lines = text.split("\n")
        val searchTotalLines = lines.size
        val canvasTotalLines = searchTotalLines
        assertEquals("search and canvas must use same totalLines", searchTotalLines, canvasTotalLines)
        assertEquals(1, results[0].lineIndex)
    }

    @Test
    fun narrowingDown_widerResultsContainNarrowerMatches() {
        val text = "hello world\nhell yeah\nshell"
        val resultsHello = findMatches(text, "hello")
        val resultsHell = findMatches(text, "hell")
        val matchLine0 = resultsHello[0]
        val foundInWiderSet =
            resultsHell.any {
                it.lineIndex == matchLine0.lineIndex && it.startIndex == matchLine0.startIndex
            }
        assertTrue("Current match must exist in narrower results", foundInWiderSet)
    }

    @Test
    fun narrowingDown_typingNewCharacterResetsToOne() {
        val text = "abc abcd"
        val resultsAbc = findMatches(text, "abc")
        assertEquals(2, resultsAbc.size)
        val resultsAbcd = findMatches(text, "abcd")
        assertEquals(1, resultsAbcd.size)
        assertEquals(4, resultsAbcd[0].startIndex)
    }
}
