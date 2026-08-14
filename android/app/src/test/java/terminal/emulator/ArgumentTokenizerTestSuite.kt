package terminal.emulator

import de.infix.testBalloon.framework.core.testSuite
import org.junit.Assert.assertEquals
import terminal.emulator.util.ArgumentTokenizer

/**
 * TestBalloon DSL-based test suite.
 *
 * TestBalloon (infix-de/testBalloon) provides a Kotlin-first DSL:
 * `val suite by testSuite { test("name") { ... } }` with coroutine
 * support, fixtures and parallel execution. The Gradle plugin
 * (`de.infix.testBalloon`) discovers top-level suites and runs them via
 * the familiar `test` tasks.
 *
 * This suite exercises the ArgumentTokenizer four-state parser
 * (termux-app port) as pure-JVM unit tests. All cases were migrated
 * from the JUnit `ArgumentTokenizerTest` so the tokenizer is covered
 * exactly once.
 */
val ArgumentTokenizerTestSuite by testSuite("ArgumentTokenizer") {

    test("plain string splits on whitespace") {
        assertEquals(listOf("echo", "hello", "world"), ArgumentTokenizer.tokenize("echo hello world"))
    }

    test("multiple whitespace collapses") {
        assertEquals(listOf("a", "b"), ArgumentTokenizer.tokenize("  a   b  "))
    }

    test("empty string yields no tokens") {
        assertEquals(emptyList<String>(), ArgumentTokenizer.tokenize(""))
        assertEquals(emptyList<String>(), ArgumentTokenizer.tokenize("   "))
    }

    test("single quotes group raw text with no escapes") {
        // Inside single quotes `\` is a plain character (DrJava rule):
        // the quote after `it\` closes, `s` continues the same token, and
        // the final `'` opens an unclosed quote kept in the token.
        // Verified against termux-app ArgumentTokenizer.java:
        // tokenize("'it\\'s'") -> [it\s]
        assertEquals(listOf("it\\s"), ArgumentTokenizer.tokenize("'it\\'s'"))
        assertEquals(listOf("hello world"), ArgumentTokenizer.tokenize("'hello world'"))
    }

    test("double quotes escape only quote and backslash") {
        assertEquals(listOf("say \"hi\""), ArgumentTokenizer.tokenize("\"say \\\"hi\\\"\""))
        // `\n` inside double quotes stays literal backslash-n.
        assertEquals(listOf("a\\nb"), ArgumentTokenizer.tokenize("\"a\\nb\""))
        // A lone `\` before the closing quote escapes the quote itself
        // (leaving the quote inside the token, quote stays open).
        assertEquals(listOf("a\""), ArgumentTokenizer.tokenize("\"a\\\""))
    }

    test("backslash outside quotes escapes next char") {
        assertEquals(listOf("hello world"), ArgumentTokenizer.tokenize("hello\\ world"))
        assertEquals(listOf("a\\b"), ArgumentTokenizer.tokenize("a\\\\b"))
    }

    test("unclosed quote kept silently") {
        // termux-app original: tokenize("'it's") -> [its] (the closing
        // quote does not split the token; `s` continues it).
        assertEquals(listOf("its"), ArgumentTokenizer.tokenize("'it's"))
        assertEquals(listOf("say", "hi"), ArgumentTokenizer.tokenize("say \"hi"))
    }

    test("no variable expansion or metacharacters") {
        assertEquals(
            listOf("\$HOME", ";", "|", "&&", "*"),
            ArgumentTokenizer.tokenize("\$HOME ; | && *"),
        )
    }

    test("stringify round-trips with escaping") {
        val args = ArgumentTokenizer.tokenize("hello world 'quoted' \"dq\"")
        val stringified = ArgumentTokenizer.tokenize(args.joinToString(" ") { it }, true)
        assertEquals(args, ArgumentTokenizer.tokenize(stringified.joinToString(" "), false))
    }

    test("mixed quotes and escapes") {
        assertEquals(
            listOf("a", "b c", "d\"e"),
            ArgumentTokenizer.tokenize("a 'b c' \"d\\\"e\""),
        )
    }

    test("empty argument from adjacent quotes") {
        assertEquals(listOf(""), ArgumentTokenizer.tokenize("''"))
        assertEquals(listOf("a", "", "b"), ArgumentTokenizer.tokenize("a '' b"))
    }

    // ── b: edge-case coverage (spec d16) ─────────────────────

    test("trailing backslash inside double quotes does not crash") {
        // torvox guard: upstream termux-app throws
        // StringIndexOutOfBoundsException on `"a\`; we keep the backslash
        // literally and the token survives.
        assertEquals(listOf("a\\"), ArgumentTokenizer.tokenize("\"a\\"))
    }

    test("trailing backslash outside quotes kept literally") {
        assertEquals(listOf("a\\"), ArgumentTokenizer.tokenize("a\\"))
        assertEquals(listOf("\\"), ArgumentTokenizer.tokenize("\\"))
    }

    test("trailing backslash combined with unclosed quote") {
        // Single-quote: `\` inside single quotes is a plain character.
        assertEquals(listOf("a\\"), ArgumentTokenizer.tokenize("'a\\"))
    }

    test("stringify escapes tab cr backspace formfeed and newline") {
        // Real control chars survive double-quote tokenization (only `\"`
        // and `\\` escape inside), then stringify emits two-char escapes.
        assertEquals(listOf("\"a\\tb\""), ArgumentTokenizer.tokenize("\"a\tb\"", true))
        assertEquals(listOf("\"a\\rb\""), ArgumentTokenizer.tokenize("\"a\rb\"", true))
        assertEquals(listOf("\"a\\bb\""), ArgumentTokenizer.tokenize("\"a\u0008b\"", true))
        assertEquals(listOf("\"a\\fb\""), ArgumentTokenizer.tokenize("\"a\u000Cb\"", true))
        assertEquals(listOf("\"a\\nb\""), ArgumentTokenizer.tokenize("\"a\nb\"", true))
        // Quote and backslash double-escape.
        assertEquals(listOf("\"a\\\"b\""), ArgumentTokenizer.tokenize("\"a\\\"b\"", true))
        assertEquals(listOf("\"a\\\\b\""), ArgumentTokenizer.tokenize("\"a\\\\b\"", true))
    }

    test("stringify output has direct format assertions") {
        // Single-quoted input keeps whitespace and control chars literal
        // (no escapes inside single quotes), then stringify quotes each
        // token and escapes \\ " \n \t \r \b \f.
        assertEquals(
            listOf(
                "\"a b\"",
                "\"c\\\"d\"",
                "\"e\\\\f\"",
                "\"g\\\\th\"",
                "\"i\\nj\"",
            ),
            ArgumentTokenizer.tokenize("'a b' 'c\"d' 'e\\f' 'g\\th' 'i\nj'", true),
        )
        // A literal backslash-quote survives the stringify round-trip.
        val arg = ArgumentTokenizer.tokenize("'a\\\"b'", false)
        val stringified = ArgumentTokenizer.tokenize("'a\\\"b'", true)
        val reparsed = ArgumentTokenizer.tokenize(stringified[0], false)
        assertEquals(arg, reparsed)
    }

    test("tab acts as separator outside quotes") {
        assertEquals(listOf("a", "b"), ArgumentTokenizer.tokenize("a\tb"))
        assertEquals(listOf("a", "b", "c"), ArgumentTokenizer.tokenize("a\t\tb c"))
    }

    test("empty quote pair inside token yields no empty argument") {
        // An empty quote pair is a state switch with no characters between
        // the quotes: the token continues without an empty argument
        // (termux-app behavior: `a""b` -> `ab`).
        assertEquals(listOf("ab"), ArgumentTokenizer.tokenize("a\"\"b"))
        assertEquals(listOf("ab"), ArgumentTokenizer.tokenize("a''b"))
    }

    test("cross-quote characters are literal") {
        assertEquals(listOf("a\"b"), ArgumentTokenizer.tokenize("'a\"b'"))
        assertEquals(listOf("a'b"), ArgumentTokenizer.tokenize("\"a'b\""))
    }

    test("unicode tokens preserved unchanged") {
        assertEquals(listOf("echo", "你好", "→"), ArgumentTokenizer.tokenize("echo 你好 →"))
        assertEquals(listOf("café"), ArgumentTokenizer.tokenize("\"café\""))
        assertEquals(listOf("日本語"), ArgumentTokenizer.tokenize("'日本語'"))
    }
}
