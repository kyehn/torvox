package io.torvox.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FindMatchesTest {
    @Test
    fun findMatches_emptyQuery_returnsEmpty() {
        val results = findMatches("hello world", "")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatches_singleMatch() {
        val results = findMatches("hello world", "world")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(6, results[0].startIndex)
        assertEquals(11, results[0].endIndex)
    }

    @Test
    fun findMatches_multipleMatchesOnSameLine() {
        val results = findMatches("abc abc abc", "abc")
        assertEquals(3, results.size)
        assertEquals(0, results[0].startIndex)
        assertEquals(4, results[1].startIndex)
        assertEquals(8, results[2].startIndex)
    }

    @Test
    fun findMatches_multipleMatchesOnDifferentLines() {
        val results = findMatches("line1 test\nline2 test\nline3 test", "test")
        assertEquals(3, results.size)
        assertEquals(0, results[0].lineIndex)
        assertEquals(1, results[1].lineIndex)
        assertEquals(2, results[2].lineIndex)
    }

    @Test
    fun findMatches_caseInsensitive() {
        val results = findMatches("Hello HELLO hello", "hello")
        assertEquals(3, results.size)
    }

    @Test
    fun findMatches_caseSensitive() {
        val results = findMatches("Hello HELLO hello", "Hello", matchCase = true)
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
    }

    @Test
    fun findMatches_noMatch() {
        val results = findMatches("hello world", "xyz")
        assertTrue(results.isEmpty())
    }

    @Test
    fun findMatches_cjkCharacters() {
        val results = findMatches("中文测试 日本語 韩国语", "测试")
        assertEquals(1, results.size)
        assertEquals(0, results[0].lineIndex)
    }

    @Test
    fun findMatches_cjkCaseInsensitive() {
        val results = findMatches("Hello 中文 World", "hello")
        assertEquals(1, results.size)
    }

    @Test
    fun findMatches_specialCharacters() {
        val results = findMatches("foo+bar+baz", "+")
        assertEquals(2, results.size)
        assertEquals(3, results[0].startIndex)
        assertEquals(7, results[1].startIndex)
    }

    @Test
    fun findMatches_parameter_passesMatchCaseToQuery() {
        val results = findMatches("Hello HELLO hello", "hello", matchCase = false)
        assertEquals(3, results.size)
    }

    @Test
    fun findMatches_parameter_whenMatchCaseFalse_findsAll() {
        val results = findMatches("Hello HELLO hello", "Hello", matchCase = true)
        assertEquals(1, results.size)
        assertEquals(0, results[0].startIndex)
    }

    @Test
    fun findMatches_parameter_whenMatchCaseTrue_findsExact() {
        val results = findMatches("Hello HELLO hello", "hello", matchCase = true)
        assertEquals(1, results.size)
        assertEquals(12, results[0].startIndex)
    }
}
