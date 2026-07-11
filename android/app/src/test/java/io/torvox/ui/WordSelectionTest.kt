package io.torvox.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class WordSelectionTest {
    // isWordChar

    @Test
    fun isWordChar_letters_true() {
        assertTrue(isWordChar('a'))
        assertTrue(isWordChar('Z'))
    }

    @Test
    fun isWordChar_digits_true() {
        assertTrue(isWordChar('0'))
        assertTrue(isWordChar('9'))
    }

    @Test
    fun isWordChar_underscore_true() {
        assertTrue(isWordChar('_'))
    }

    @Test
    fun isWordChar_dot_hyphen_slash_true() {
        assertTrue(isWordChar('.'))
        assertTrue(isWordChar('-'))
        assertTrue(isWordChar('/'))
    }

    @Test
    fun isWordChar_punctuation_false() {
        assertFalse(isWordChar(')'))
        assertFalse(isWordChar(','))
        assertFalse(isWordChar(':'))
        assertFalse(isWordChar(';'))
    }

    @Test
    fun isWordChar_whitespace_false() {
        assertFalse(isWordChar(' '))
        assertFalse(isWordChar('\t'))
    }

    // expandWordOnLine

    @Test
    fun expandWordOnLine_simpleWord() {
        val result = expandWordOnLine("hello_world", 4)
        assertEquals(0, result.first)
        assertEquals(11, result.second)
    }

    @Test
    fun expandWordOnLine_punctuationBoundary() {
        val result = expandWordOnLine("hello)world", 4)
        assertEquals(0, result.first)
        assertEquals(5, result.second)
    }

    @Test
    fun expandWordOnLine_onDigit() {
        val result = expandWordOnLine("abc123def", 5)
        assertEquals(0, result.first)
        assertEquals(9, result.second)
    }

    @Test
    fun expandWordOnLine_underscoreIsWordChar() {
        val result = expandWordOnLine("hello_world", 5)
        assertEquals(0, result.first)
        assertEquals(11, result.second)
    }

    @Test
    fun expandWordOnLine_onWhitespaceNearestRight() {
        val result = expandWordOnLine("hello world", 6)
        assertEquals(6, result.first)
        assertEquals(11, result.second)
    }

    @Test
    fun expandWordOnLine_onWhitespaceNearestLeft() {
        val result = expandWordOnLine("hello  world", 7)
        assertEquals(7, result.first)
        assertEquals(12, result.second)
    }

    @Test
    fun expandWordOnLine_onPunctuationNearest() {
        val result = expandWordOnLine("foo.bar", 3)
        assertEquals(0, result.first)
        assertEquals(7, result.second)
    }

    @Test
    fun expandWordOnLine_onBoundaryNoWord() {
        val result = expandWordOnLine("!!!", 1)
        assertEquals(1, result.first)
        assertEquals(1, result.second)
    }

    @Test
    fun expandWordOnLine_edgeStart() {
        val result = expandWordOnLine("hello", 0)
        assertEquals(0, result.first)
        assertEquals(5, result.second)
    }

    @Test
    fun expandWordOnLine_edgeEnd() {
        val result = expandWordOnLine("hello", 4)
        assertEquals(0, result.first)
        assertEquals(5, result.second)
    }

    @Test
    fun expandWordOnLine_emptyLine() {
        val result = expandWordOnLine("", 0)
        assertEquals(0, result.first)
        assertEquals(0, result.second)
    }

    @Test
    fun expandWordOnLine_colBeyondLength() {
        val result = expandWordOnLine("hi", 5)
        assertEquals(5, result.first)
        assertEquals(5, result.second)
    }

    @Test
    fun expandWordOnLine_urlDoesNotExpandPastPunctuation() {
        val result = expandWordOnLine("visit https://example.com now", 8)
        assertEquals(6, result.first)
        assertEquals(11, result.second)
    }

    @Test
    fun expandWordOnLine_slashIsPartOfPath() {
        val result = expandWordOnLine("cd /usr/local/bin", 4)
        assertEquals(3, result.first)
        assertEquals(17, result.second)
    }

    @Test
    fun expandWordOnLine_punctuationSeparatesWords() {
        val result = expandWordOnLine("foo(bar)baz", 4)
        assertEquals(4, result.first)
        assertEquals(7, result.second)
    }
}
