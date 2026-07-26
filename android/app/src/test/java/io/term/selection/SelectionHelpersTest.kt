package io.term.selection

import io.term.ui.expandWordOnLine
import io.term.ui.isWordChar
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SelectionHelpersTest {
    // -- isWordChar --

    @Test
    fun isWordChar_letters_true() {
        assertTrue(isWordChar('a'))
        assertTrue(isWordChar('Z'))
        assertTrue(isWordChar('n'))
    }

    @Test
    fun isWordChar_digits_true() {
        assertTrue(isWordChar('0'))
        assertTrue(isWordChar('9'))
    }

    @Test
    fun isWordChar_underscore_hyphen_dot_slash_true() {
        assertTrue(isWordChar('_'))
        assertTrue(isWordChar('-'))
        assertTrue(isWordChar('.'))
        assertTrue(isWordChar('/'))
    }

    @Test
    fun isWordChar_bracket_exclamation_false() {
        assertFalse(isWordChar(':'))
        assertFalse(isWordChar('@'))
        assertFalse(isWordChar('+'))
        assertFalse(isWordChar(')'))
        assertFalse(isWordChar(','))
        assertFalse(isWordChar(';'))
    }

    @Test
    fun isWordChar_whitespace_false() {
        assertFalse(isWordChar(' '))
        assertFalse(isWordChar('\t'))
        assertFalse(isWordChar('\n'))
    }

    @Test
    fun isWordChar_brackets_special_false() {
        assertFalse(isWordChar('('))
        assertFalse(isWordChar(')'))
        assertFalse(isWordChar('['))
        assertFalse(isWordChar(']'))
        assertFalse(isWordChar('{'))
        assertFalse(isWordChar('}'))
        assertFalse(isWordChar('<'))
        assertFalse(isWordChar('>'))
        assertFalse(isWordChar('"'))
        assertFalse(isWordChar('\''))
        assertFalse(isWordChar('`'))
        assertFalse(isWordChar('|'))
        assertFalse(isWordChar('\\'))
        assertFalse(isWordChar(','))
        assertFalse(isWordChar(';'))
        assertFalse(isWordChar('!'))
    }

    // -- expandWordOnLine --
    // expandWordOnLine uses isWordChar, so URL chars (:, /, ., etc.) are word chars.
    // The result is a contiguous span of word chars around the pivot column.
    // On whitespace/punctuation, it snaps to the nearest word char.

    @Test
    fun expandWordOnLine_middleOfWord() {
        val (start, end) = expandWordOnLine("hello world terminal", 6)
        // col 6 = 'w', word is "world" at [6,11)
        assertEquals(6, start)
        assertEquals(11, end)
    }

    @Test
    fun expandWordOnLine_atWordStart() {
        val (start, end) = expandWordOnLine("hello world", 0)
        // col 0 = 'h', word "hello" at [0,5)
        assertEquals(0, start)
        assertEquals(5, end)
    }

    @Test
    fun expandWordOnLine_atWordEnd() {
        val (start, end) = expandWordOnLine("hello world", 4)
        // col 4 = 'o' (last char of "hello"), word at [0,5)
        assertEquals(0, start)
        assertEquals(5, end)
    }

    @Test
    fun expandWordOnLine_onWhitespaceBetweenWords() {
        val (start, end) = expandWordOnLine("abc def", 3)
        // col 3 = space, snaps to nearest word char: left 'c' at col 2
        // pivot becomes 2, word "abc" at [0,3)
        assertEquals(0, start)
        assertEquals(3, end)
    }

    @Test
    fun expandWordOnLine_atLineFirstChar() {
        val (start, end) = expandWordOnLine("sample", 0)
        // col 0 = 's', word at [0,6)
        assertEquals(0, start)
        assertEquals(6, end)
    }

    @Test
    fun expandWordOnLine_atLineLastChar() {
        val (start, end) = expandWordOnLine("sample", 5)
        // col 5 = 'e', word at [0,6)
        assertEquals(0, start)
        assertEquals(6, end)
    }

    @Test
    fun expandWordOnLine_urlDoesNotExpandPastNextWord() {
        val (start, end) = expandWordOnLine("hello world", 8)
        // col 8 = 'r' in "world", word at [6,11)
        assertEquals(6, start)
        assertEquals(11, end)
    }

    @Test
    fun expandWordOnLine_pathSeparator() {
        val (start, end) = expandWordOnLine("cd /usr/local/bin", 7)
        // col 7 = '/' is word char, spans from '/' at col 3 to 'n' at col 16 (exclusive: 17)
        assertEquals(3, start)
        assertEquals(17, end)
    }

    @Test
    fun expandWordOnLine_punctuationBoundary() {
        val (start, end) = expandWordOnLine("hello(world)baz", 6)
        // col 6 = 'w', '(' and ')' are not word chars, so word "world" at [6,11)
        assertEquals(6, start)
        assertEquals(11, end)
    }

    @Test
    fun expandWordOnLine_underscoreIsWordChar() {
        val (start, end) = expandWordOnLine("my_var_name", 4)
        // col 4 = 'a', all word chars, whole line at [0,11)
        assertEquals(0, start)
        assertEquals(11, end)
    }

    @Test
    fun expandWordOnLine_emptyLine() {
        val (start, end) = expandWordOnLine("", 0)
        // 0 >= line.length=0, returns Pair(0, 0)
        assertEquals(0, start)
        assertEquals(0, end)
    }

    @Test
    fun expandWordOnLine_colBeyondLength() {
        val (start, end) = expandWordOnLine("hi", 10)
        // 10 >= line.length=2, returns Pair(10, 10)
        assertEquals(10, start)
        assertEquals(10, end)
    }

    // -- detectUrlBounds (uses same regex as TerminalSurface.detectUrlBounds) --

    private val urlRegex =
        Regex("(https?://[^\\s<>\"'`\\\\|\\[\\]{}]+|www\\.[^\\s<>\"'`\\\\|\\[\\]{}]+)")

    private fun detectUrlBounds(
        line: String,
        col: Int,
    ): Pair<Int, Int>? {
        var bestMatch: Pair<Int, Int>? = null
        var bestDist = Int.MAX_VALUE
        for (match in urlRegex.findAll(line)) {
            if (col >= match.range.first - 1 && col <= match.range.last + 1) {
                val dist = minOf(kotlin.math.abs(col - match.range.first), kotlin.math.abs(col - match.range.last))
                if (dist < bestDist) {
                    bestDist = dist
                    bestMatch = Pair(match.range.first, match.range.last)
                }
            }
        }
        return bestMatch
    }

    @Test
    fun detectUrlBounds_simpleHttpUrl() {
        val line = "visit https://example.com/path now"
        // "https://example.com/path" at cols 6..29
        val bounds = detectUrlBounds(line, 8)
        assertNotNull(bounds)
        assertEquals(6, bounds!!.first)
        assertEquals(29, bounds.second)
    }

    @Test
    fun detectUrlBounds_middleOfHttpsUrl() {
        val line = "check https://secure.example.com/page?id=123 now"
        // "https://secure.example.com/page?id=123" at cols 6..? (len=36, lastIdx=41)
        val bounds = detectUrlBounds(line, 10)
        assertNotNull(bounds)
        assertTrue(bounds!!.first <= 10)
        assertTrue(bounds.second >= 41)
    }

    @Test
    fun detectUrlBounds_wwwDotUrl() {
        val line = "go to www.example.com/path now"
        val bounds = detectUrlBounds(line, 8)
        assertNotNull(bounds)
        assertTrue(bounds!!.first <= 8)
        assertTrue(bounds.second >= 22)
    }

    @Test
    fun detectUrlBounds_httpUrlWithPort() {
        val line = "http://localhost:8080/api/v1/test"
        val bounds = detectUrlBounds(line, 10)
        assertNotNull(bounds)
        assertEquals(0, bounds!!.first)
        assertEquals(32, bounds.second)
    }

    @Test
    fun detectUrlBounds_atLineStart_col0() {
        val bounds = detectUrlBounds("https://example.com", 0)
        assertNotNull(bounds)
        assertEquals(0, bounds!!.first)
        assertEquals(18, bounds.second)
    }

    @Test
    fun detectUrlBounds_onePastUrlEdge() {
        // URL spans 0..18, buffer is 0..19. col=20 is outside.
        val bounds = detectUrlBounds("https://example.com", 20)
        assertNull(bounds)
    }

    @Test
    fun detectUrlBounds_atBufferEdge() {
        // URL spans 0..18, col=19 is within 1-column buffer.
        val bounds = detectUrlBounds("https://example.com", 19)
        assertNotNull(bounds)
        assertEquals(0, bounds!!.first)
        assertEquals(18, bounds.second)
    }

    @Test
    fun detectUrlBounds_farFromUrl_returnsNull() {
        val bounds = detectUrlBounds("not a url at all", 5)
        assertNull(bounds)
    }

    @Test
    fun detectUrlBounds_multipleUrls_picksNearest() {
        val bounds = detectUrlBounds("first https://first.com and https://second.com/path", 40)
        assertNotNull(bounds)
        assertTrue(bounds!!.first >= 0)
    }

    @Test
    fun detectUrlBounds_urlWithQueryString() {
        val line = "search https://example.com/search?q=test&page=2"
        val bounds = detectUrlBounds(line, 10)
        assertNotNull(bounds)
        assertEquals(7, bounds!!.first)
    }

    @Test
    fun detectUrlBounds_urlWithFragment() {
        val bounds = detectUrlBounds("link https://example.com/page#section", 10)
        assertNotNull(bounds)
        assertEquals(5, bounds!!.first)
    }

    @Test
    fun detectUrlBounds_urlWithPathSpecialChars() {
        val bounds = detectUrlBounds("url https://example.com/~user/path+test", 10)
        assertNotNull(bounds)
        assertEquals(4, bounds!!.first)
    }

    @Test
    fun detectUrlBounds_colBeforeUrlMinusTwo() {
        // URL spans 0..18, buffer starts at -1. col=-2 is outside.
        val bounds = detectUrlBounds("https://example.com", -2)
        assertNull(bounds)
    }

    @Test
    fun detectUrlBounds_emptyLine() {
        val bounds = detectUrlBounds("", 0)
        assertNull(bounds)
    }
}
