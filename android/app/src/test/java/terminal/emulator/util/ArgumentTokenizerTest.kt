package terminal.emulator.util

import org.junit.Assert.assertEquals
import org.junit.Test

class ArgumentTokenizerTest {
    @Test
    fun `plain string splits on whitespace`() {
        assertEquals(listOf("echo", "hello", "world"), ArgumentTokenizer.tokenize("echo hello world"))
    }

    @Test
    fun `multiple whitespace collapses`() {
        assertEquals(listOf("a", "b"), ArgumentTokenizer.tokenize("  a   b  "))
    }

    @Test
    fun `empty string yields no tokens`() {
        assertEquals(emptyList<String>(), ArgumentTokenizer.tokenize(""))
        assertEquals(emptyList<String>(), ArgumentTokenizer.tokenize("   "))
    }

    @Test
    fun `single quotes group raw text with no escapes`() {
        // Inside single quotes `\` is a plain character (DrJava rule),
        // so the token keeps the backslash.
        assertEquals(listOf("it\\", "s"), ArgumentTokenizer.tokenize("'it\\'s'"))
        assertEquals(listOf("hello world"), ArgumentTokenizer.tokenize("'hello world'"))
    }

    @Test
    fun `double quotes escape only quote and backslash`() {
        assertEquals(listOf("say \"hi\""), ArgumentTokenizer.tokenize("\"say \\\"hi\\\"\""))
        // `\n` inside double quotes stays literal backslash-n.
        assertEquals(listOf("a\\nb"), ArgumentTokenizer.tokenize("\"a\\nb\""))
        // A lone `\` before end inside quotes stays.
        assertEquals(listOf("a\\"), ArgumentTokenizer.tokenize("\"a\\\""))
    }

    @Test
    fun `backslash outside quotes escapes next char`() {
        assertEquals(listOf("hello world"), ArgumentTokenizer.tokenize("hello\\ world"))
        assertEquals(listOf("a\\b"), ArgumentTokenizer.tokenize("a\\\\b"))
    }

    @Test
    fun `unclosed quote kept silently`() {
        assertEquals(listOf("it's"), ArgumentTokenizer.tokenize("'it's"))
        assertEquals(listOf("say hi"), ArgumentTokenizer.tokenize("say \"hi"))
    }

    @Test
    fun `no variable expansion or metacharacters`() {
        assertEquals(listOf("$HOME", ";", "|", "&&", "*"), ArgumentTokenizer.tokenize("$HOME ; | && *"))
    }

    @Test
    fun `stringify round-trips with escaping`() {
        val args = ArgumentTokenizer.tokenize("hello world 'quoted' \"dq\"")
        val stringified = ArgumentTokenizer.tokenize(args.joinToString(" ") { it }, true)
        assertEquals(args, ArgumentTokenizer.tokenize(stringified.joinToString(" "), false))
    }

    @Test
    fun `mixed quotes and escapes`() {
        assertEquals(
            listOf("a", "b c", "d\"e"),
            ArgumentTokenizer.tokenize("a 'b c' \"d\\\"e\""),
        )
    }

    @Test
    fun `empty argument from adjacent quotes`() {
        assertEquals(listOf(""), ArgumentTokenizer.tokenize("''"))
        assertEquals(listOf("a", "", "b"), ArgumentTokenizer.tokenize("a '' b"))
    }
}