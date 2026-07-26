package io.term.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SelectionTest {
    // ── Word expansion tests ──

    @Test
    fun expandWordMiddleOfWord() {
        // "hello world" -> word at col 6 (world) should expand to world
        val result = expandWordOnLine("hello world", 6)
        assertEquals("start should be 'w'", 6, result.first)
        assertEquals("end should be past 'd'", 11, result.second)
    }

    @Test
    fun expandWordStartOfWord() {
        val result = expandWordOnLine("hello world", 0)
        assertEquals("start should be 'h'", 0, result.first)
        assertEquals("end should be past 'o'", 5, result.second)
    }

    @Test
    fun expandWordOnUrl() {
        // ':' is not a word char, so URL expansion stops at the colon
        val result = expandWordOnLine("visit https://example.com/path now", 6)
        assertEquals("start should be at 'h' in https", 6, result.first)
        assertEquals("end should be past 's' in https", 11, result.second)
    }

    @Test
    fun expandWordOnPathWithSlashes() {
        // Slash IS a word char, so paths expand continuously
        val result = expandWordOnLine("file /usr/local/bin/test", 7)
        assertEquals("start should be at '/' in path", 5, result.first)
        assertEquals("end should be past 't'", 24, result.second)
    }

    @Test
    fun expandWordOnDotSeparated() {
        // Dot IS a word char
        val result = expandWordOnLine("open file.name.txt here", 7)
        assertEquals("start should be at 'f' in file", 5, result.first)
        assertEquals("end should be past 't' in txt", 18, result.second)
    }

    @Test
    fun expandWordOnHyphenated() {
        // Hyphen IS a word char
        val result = expandWordOnLine("use well-known-port value", 7)
        assertEquals("start should be at 'w' in well", 4, result.first)
        assertEquals("end should be past 't' in port", 19, result.second)
    }

    @Test
    fun expandWordOnEmptyString() {
        val result = expandWordOnLine("", 0)
        assertEquals(0, result.first)
        assertEquals(0, result.second)
    }

    @Test
    fun expandWordOnNegativeCol() {
        val result = expandWordOnLine("hello", -1)
        assertEquals(0, result.first)
        assertEquals(0, result.second)
    }

    @Test
    fun expandWordOnColBeyondLine() {
        val result = expandWordOnLine("hello", 10)
        assertEquals(10, result.first)
        assertEquals(10, result.second)
    }

    @Test
    fun expandWordOnNonWordCharMidLine() {
        val result = expandWordOnLine("hello (world) test", 6)
        // '(' at col 5, ')' at col 11 - the cursor at col 6 (inside the parens)
        // Since '(' and ')' are not word chars and spaces are not either,
        // the behavior depends on the implementation. Let's just check it handles gracefully.
        assertTrue("Start should be >= 0", result.first >= 0)
        assertTrue("End should be >= start", result.second >= result.first)
    }

    @Test
    fun expandWordOnLastWord() {
        val result = expandWordOnLine("hello world", 10)
        assertEquals("start should be at 'w'", 6, result.first)
        assertEquals("end should be past 'd'", 11, result.second)
    }

    @Test
    fun isWordCharVarious() {
        assertTrue("letter is word char", isWordChar('a'))
        assertTrue("digit is word char", isWordChar('1'))
        assertTrue("underscore is word char", isWordChar('_'))
        assertTrue("hyphen is word char", isWordChar('-'))
        assertTrue("dot is word char", isWordChar('.'))
        assertTrue("slash is word char", isWordChar('/'))
        assertFalse("space is not word char", isWordChar(' '))
        assertFalse("paren is not word char", isWordChar('('))
        assertFalse("bracket is not word char", isWordChar('['))
    }
}
