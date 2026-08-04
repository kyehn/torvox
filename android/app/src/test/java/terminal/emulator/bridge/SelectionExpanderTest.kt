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
        assertEquals(0 to 4, SelectionExpander.expandBounds("hello world", 2))
    }

    @Test
    fun `second word expands independently`() {
        assertEquals(6 to 10, SelectionExpander.expandBounds("hello world", 8))
    }

    @Test
    fun `tap on whitespace expands whole line run`() {
        // Tap on the space itself: both sides are non-whitespace, so the
        // run covers the whole "hello world" (0..10).
        assertEquals(0 to 10, SelectionExpander.expandBounds("hello world", 5))
    }

    @Test
    fun `url with scheme expands to whole url`() {
        val line = "see https://example.com/path?q=1 more"
        // URL = cols 4..31 (space at 32)
        assertEquals(4 to 31, SelectionExpander.expandBounds(line, 10))
    }

    @Test
    fun `tap at end of url expands to whole url`() {
        val line = "see https://example.com/path?q=1 more"
        assertEquals(4 to 31, SelectionExpander.expandBounds(line, 31))
    }

    @Test
    fun `path with slashes expands as one word`() {
        // "path/to/file.txt" = cols 0..15
        assertEquals(0 to 15, SelectionExpander.expandBounds("path/to/file.txt x", 6))
    }

    @Test
    fun `tap after url on following word selects that word only`() {
        val line = "see https://example.com more words"
        // "more" = cols 24..27 (space at 23 and 28)
        assertEquals(24 to 27, SelectionExpander.expandBounds(line, 26))
    }

    @Test
    fun `quoted string expands to inner word`() {
        val line = "echo \"hello world\" done"
        // Tap inside "hello": trimmed run = cols 6..10
        assertEquals(6 to 10, SelectionExpander.expandBounds(line, 10))
    }

    @Test
    fun `bracketed word trims brackets on tap side`() {
        val line = "run (build) now"
        // run = 4..10 ("(build)"); tap at 'u' (col 6) inside:
        // opening bracket trimmed (c > start), closing bracket trimmed
        // (c < end) -> 5..9
        assertEquals(5 to 9, SelectionExpander.expandBounds(line, 6))
    }

    @Test
    fun `tap on closing bracket trims only opening bracket`() {
        val line = "run (build) now"
        // Tap at ')' (col 10): run = 4..10; opening bracket trimmed
        // (c > start), closing bracket kept (c == end) -> 5..10
        assertEquals(5 to 10, SelectionExpander.expandBounds(line, 10))
    }

    @Test
    fun `col beyond line clamps to last char`() {
        assertEquals(0 to 4, SelectionExpander.expandBounds("hello", 99))
    }

    @Test
    fun `negative col clamps to first char`() {
        assertEquals(0 to 4, SelectionExpander.expandBounds("hello", -3))
    }

    @Test
    fun `single char line`() {
        assertEquals(0 to 0, SelectionExpander.expandBounds("x", 0))
    }
}
