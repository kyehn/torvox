package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class TextSearchFindMatchesTest {
    @Test
    fun `finds single match`() {
        val results = findMatches("hello world", "world")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(6, results[0].startIndex)
        assertEquals(11, results[0].endIndex)
    }

    @Test
    fun `empty query returns nothing`() {
        assertEquals(emptyList<SearchResult>(), findMatches("abc", ""))
    }

    @Test
    fun `case insensitive by default`() {
        // All three forms match case-insensitively
        assertEquals(3, findMatches("Hello HELLO hello", "hello").size)
    }

    @Test
    fun `case sensitive mode`() {
        // Only the exact lowercase form matches
        assertEquals(1, findMatches("Hello hello HELLO", "hello", matchCase = true).size)
    }

    @Test
    fun `multiple matches per line`() {
        val results = findMatches("aa aa aa", "aa")
        assertEquals(3, results.size)
        assertEquals(listOf(0, 3, 6), results.map { it.startIndex })
    }

    @Test
    fun `multi line results carry line index`() {
        val results = findMatches("one\ntwo one\nthree", "one")
        assertEquals(2, results.size)
        assertEquals(listOf(0, 1), results.map { it.lineIndex })
    }

    @Test
    fun `cjk query matches`() {
        // charIndexToCellColumn: CJK chars are double-width, so "你好"
        // occupies columns 0-3, space = 4, "世界" starts at column 5.
        val results = findMatches("你好 世界", "世界")
        assertEquals(1, results.size)
        assertEquals(5, results[0].startIndex)
        assertEquals(9, results[0].endIndex)
    }

    @Test
    fun `no match returns empty`() {
        assertEquals(emptyList<SearchResult>(), findMatches("abc", "xyz"))
    }
}
