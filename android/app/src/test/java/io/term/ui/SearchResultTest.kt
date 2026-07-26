package io.term.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SearchResultTest {
    @Test
    fun searchResultData() {
        val r = SearchResult(lineIndex = 5, startIndex = 10, endIndex = 20)
        assertEquals(5, r.lineIndex)
        assertEquals(10, r.startIndex)
        assertEquals(20, r.endIndex)
    }

    @Test
    fun searchResultEquality() {
        val a = SearchResult(lineIndex = 1, startIndex = 2, endIndex = 3)
        val b = SearchResult(lineIndex = 1, startIndex = 2, endIndex = 3)
        assertEquals(a, b)
    }

    @Test
    fun searchResultInequality() {
        val a = SearchResult(lineIndex = 1, startIndex = 2, endIndex = 3)
        val b = SearchResult(lineIndex = 2, startIndex = 2, endIndex = 3)
        assertNotEquals(a, b)
    }

    @Test
    fun findMatches_singleLine_singleMatch() {
        val results = findMatches("Hello World", "World")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(6, results[0].startIndex)
        assertEquals(11, results[0].endIndex)
    }

    @Test
    fun findMatches_singleLine_noMatch() {
        val results = findMatches("Hello World", "xyz")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatches_emptyQuery_emptyResult() {
        val results = findMatches("Hello World", "")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatches_emptyText_emptyResult() {
        val results = findMatches("", "hello")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatches_caseSensitive() {
        val results = findMatches("Hello HELLO hello", "hello", matchCase = true)
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(12, results[0].startIndex) // lowercase 'h' in 'hello' at char index 12
        assertEquals(17, results[0].endIndex)
    }

    @Test
    fun findMatches_caseSensitive_multiline() {
        val results = findMatches("Hello\nHELLO\nhello", "hello", matchCase = true)
        assertEquals(1, results.size)
        assertEquals(2, results[0].lineIndex) // only matches the last line
    }

    @Test
    fun findMatches_caseInsensitive() {
        val results = findMatches("Hello HELLO hello", "hello", matchCase = false)
        assertEquals(3, results.size)
    }

    @Test
    fun findMatches_multipleLines() {
        val results = findMatches("abc\n123\nxyz", "123")
        assertEquals(1, results.size)
        assertEquals(1, results[0].lineIndex)
        assertEquals(0, results[0].startIndex)
        assertEquals(3, results[0].endIndex)
    }

    @Test
    fun findMatches_acrossLines_distinctResults() {
        val text = "aa\naa\naa"
        val results = findMatches(text, "aa")
        assertEquals(3, results.size)
    }

    @Test
    fun findMatches_overlappingConsecutive() {
        val results = findMatches("aaaa", "aa")
        assertEquals(3, results.size)
        assertEquals(0, results[0].startIndex)
        assertEquals(1, results[1].startIndex)
        assertEquals(2, results[2].startIndex)
    }

    @Test
    fun findMatches_withCJK() {
        // CJK chars are 2 cells wide
        val results = findMatches("你好世界", "世界")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        // "世界" starts at char index 2, which in cell columns = 2*2 = 4
        assertEquals(4, results[0].startIndex)
        // "世界" ends at cell column 4 + 2*2 = 8
        assertEquals(8, results[0].endIndex)
    }

    @Test
    fun findMatches_mixedWidth() {
        // "a你b好" -> cell widths: 1 + 2 + 1 + 2 = 6 cells
        val results = findMatches("a你b好", "b好")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        // "b好" starts at char index 2, cell column = 0+1+2 = 3
        assertEquals(3, results[0].startIndex)
        // ends at cell column 3 + 1 + 2 = 6
        assertEquals(6, results[0].endIndex)
    }

    @Test
    fun findMatches_includesQueryAtLineBoundaries() {
        val results = findMatches("prefix end", "end")
        assertEquals(1, results.size)
        assertEquals(7, results[0].startIndex)
        assertEquals(10, results[0].endIndex)
    }

    @Test
    fun findMatches_queryExactMatchFullLine() {
        val results = findMatches("fullmatch", "fullmatch")
        assertEquals(1, results.size)
        assertEquals(0, results[0].startIndex)
        assertEquals(9, results[0].endIndex)
    }

    // Cell width is tested indirectly via findMatches with CJK content.
    // The charCellWidth/charIndexToCellColumn functions are private and
    // only exercised through the public findMatches API.

    @Test
    fun findMatches_sameLineMultipleMatches() {
        val results = findMatches("cat dog cat", "cat")
        assertEquals(2, results.size)
        assertEquals(0, results[0].startIndex)
        assertEquals(8, results[1].startIndex)
    }

    @Test
    fun findMatches_largeText_performanceCheck() {
        val lines = (1..100).joinToString("\n") { "line $it content" }
        val results = findMatches(lines, "content")
        assertEquals(100, results.size)
    }
}
