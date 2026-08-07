package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test

/** Boundary rules for smart text selection (round-214). */
class SelectionExpanderTest {

    @Test
    fun `empty line expands to zero range`() {
        assertEquals(0 to 0, SelectionExpander.expandBounds("", 0))
    }

    @Test
    fun `single word expands to whole word`() {
        assertEquals(0 to 4, SelectionExpander.expandBounds("hello", 2))
    }

    @Test
    fun `second word expands independently`() {
        assertEquals(6 to 10, SelectionExpander.expandBounds("hello world", 8))
    }

    @Test
    fun `tap on whitespace expands whole line run`() {
        // Whitespace tap falls back to the maximal non-whitespace run.
        assertEquals(0 to 10, SelectionExpander.expandBounds("hello world", 5))
    }

    @Test
    fun `url with scheme expands to whole url`() {
        assertEquals(6 to 20, SelectionExpander.expandBounds("visit https://x.com/a now", 12))
    }

    @Test
    fun `tap at end of url expands to whole url`() {
        assertEquals(6 to 20, SelectionExpander.expandBounds("visit https://x.com/a now", 20))
    }

    @Test
    fun `path with slashes expands as one word`() {
        assertEquals(0 to 16, SelectionExpander.expandBounds("/usr/local/bin/sh", 10))
    }

    @Test
    fun `tap after url on following word selects that word only`() {
        assertEquals(22 to 24, SelectionExpander.expandBounds("visit https://x.com/a now", 23))
    }

    @Test
    fun `quoted string expands to inner word`() {
        assertEquals(1 to 5, SelectionExpander.expandBounds("\"hello\"", 3))
    }

    @Test
    fun `bracketed word trims brackets on tap side`() {
        assertEquals(1 to 5, SelectionExpander.expandBounds("(hello)", 3))
    }

    @Test
    fun `tap on closing bracket trims only opening bracket`() {
        assertEquals(1 to 6, SelectionExpander.expandBounds("(hello)", 6))
    }

    @Test
    fun `col beyond line clamps to last char`() {
        assertEquals(0 to 4, SelectionExpander.expandBounds("hello", 100))
    }

    @Test
    fun `negative col clamps to first char`() {
        assertEquals(0 to 4, SelectionExpander.expandBounds("hello", -5))
    }

    @Test
    fun `single char line`() {
        assertEquals(0 to 0, SelectionExpander.expandBounds("x", 0))
    }

    @Test
    fun `url with trailing comma trims comma`() {
        val (start, end) = SelectionExpander.expandBounds("visit https://x.com/a, now", 14)
        val selected = "visit https://x.com/a, now".substring(start, end + 1)
        assert(selected == "https://x.com/a") { "expected url without comma, got '$selected'" }
    }

    @Test
    fun `url with trailing period trims period`() {
        val (start, end) = SelectionExpander.expandBounds("see https://x.com/a. next", 14)
        val selected = "see https://x.com/a. next".substring(start, end + 1)
        assert(selected == "https://x.com/a") { "expected url without period, got '$selected'" }
    }

    @Test
    fun `url followed by unbalanced close paren trims paren`() {
        val (start, end) = SelectionExpander.expandBounds("link https://x.com/a).", 14)
        val selected = "link https://x.com/a).".substring(start, end + 1)
        assert(selected == "https://x.com/a") { "expected url without ').', got '$selected'" }
    }

    @Test
    fun `url inside balanced parens keeps parens`() {
        val (start, end) = SelectionExpander.expandBounds("(https://x.com/a)", 6)
        val selected = "(https://x.com/a)".substring(start, end + 1)
        assert(selected == "https://x.com/a") { "expected url without parens, got '$selected'" }
    }

    @Test
    fun `url with trailing semicolon trims semicolon`() {
        val (start, end) = SelectionExpander.expandBounds("x https://x.com/a; y", 14)
        val selected = "x https://x.com/a; y".substring(start, end + 1)
        assert(selected == "https://x.com/a") { "expected url without semicolon, got '$selected'" }
    }
}
