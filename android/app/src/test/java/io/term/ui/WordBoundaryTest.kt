package io.term.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Requirement 2: `expandWordOnLine` boundaries.
 *
 * Word characters are letters, digits, underscore, dot, hyphen and slash.
 * Punctuation (and whitespace) is NOT a word character. Empty string and
 * out-of-bounds columns must be handled gracefully. Mirrors the style of
 * [SelectionTest].
 */
class WordBoundaryTest {
    @Test
    fun word_char_letters() {
        assertTrue("'a' is word char", isWordChar('a'))
        assertTrue("'Z' is word char", isWordChar('Z'))
    }

    @Test
    fun word_char_digit() {
        assertTrue("'0' is word char", isWordChar('0'))
        assertTrue("'9' is word char", isWordChar('9'))
    }

    @Test
    fun word_char_underscore() {
        assertTrue("'_' is word char", isWordChar('_'))
    }

    @Test
    fun word_char_dot() {
        assertTrue("'.' is word char (URL/path)", isWordChar('.'))
    }

    @Test
    fun word_char_hyphen() {
        assertTrue("'-' is word char (path/flag)", isWordChar('-'))
    }

    @Test
    fun word_char_slash() {
        assertTrue("'/' is word char (path)", isWordChar('/'))
    }

    @Test
    fun punctuation_is_not_word_char() {
        assertFalse("comma is not word char", isWordChar(','))
        assertFalse("paren is not word char", isWordChar('('))
        assertFalse("paren is not word char", isWordChar(')'))
        assertFalse("bracket is not word char", isWordChar('['))
        assertFalse("bang is not word char", isWordChar('!'))
        assertFalse("semicolon is not word char", isWordChar(';'))
    }

    @Test
    fun whitespace_is_not_word_char() {
        assertFalse("space is not word char", isWordChar(' '))
        assertFalse("tab is not word char", isWordChar('\t'))
    }

    @Test
    fun expand_word_middle() {
        val (start, end) = expandWordOnLine("hello world", 2)
        assertEquals(0, start)
        assertEquals(5, end)
    }

    @Test
    fun expand_word_with_underscore() {
        val (start, end) = expandWordOnLine("foo_bar_baz", 4)
        assertEquals(0, start)
        assertEquals(11, end)
    }

    @Test
    fun expand_word_with_digit() {
        val line = "var123name"
        val (start, end) = expandWordOnLine(line, 4)
        assertEquals(0, start)
        assertEquals(line.length, end)
    }

    @Test
    fun expand_word_with_dot() {
        // dot is a word char -> dotted identifiers expand as one token
        val line = "com.example.foo"
        val (start, end) = expandWordOnLine(line, 4)
        assertEquals(0, start)
        assertEquals(line.length, end)
    }

    @Test
    fun expand_word_with_hyphen() {
        val line = "well-known-port"
        val (start, end) = expandWordOnLine(line, 4)
        assertEquals(0, start)
        assertEquals(line.length, end)
    }

    @Test
    fun expand_word_with_slash_path() {
        val line = "/usr/local/bin"
        val (start, end) = expandWordOnLine(line, 5)
        assertEquals(0, start)
        assertEquals(line.length, end)
    }

    @Test
    fun expand_word_stops_at_punctuation() {
        val (start, end) = expandWordOnLine("hello, world", 2)
        // comma is not a word char, so only "hello" is selected
        assertEquals(0, start)
        assertEquals(5, end)
    }

    @Test
    fun expand_word_empty_string() {
        val (start, end) = expandWordOnLine("", 0)
        assertEquals(0, start)
        assertEquals(0, end)
    }

    @Test
    fun expand_word_negative_col() {
        val (start, end) = expandWordOnLine("hello", -1)
        assertEquals(0, start)
        assertEquals(0, end)
    }

    @Test
    fun expand_word_col_beyond_length() {
        val (start, end) = expandWordOnLine("hello", 10)
        assertEquals(10, start)
        assertEquals(10, end)
    }
}
