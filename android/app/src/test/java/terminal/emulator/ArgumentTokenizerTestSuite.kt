package terminal.emulator

import de.infix.testBalloon.framework.core.testSuite
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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
 * (termux-app port) as pure-JVM unit tests.
 */
val ArgumentTokenizerTestSuite by testSuite("ArgumentTokenizer") {

    test("tokenizes command with arguments") {
        val tokens = ArgumentTokenizer.tokenize("echo hello world")
        assertEquals(listOf("echo", "hello", "world"), tokens)
    }

    test("handles quoted arguments") {
        val tokens = ArgumentTokenizer.tokenize("echo \"hello world\" 'single quoted'")
        assertEquals(listOf("echo", "hello world", "single quoted"), tokens)
    }

    test("handles escaped characters") {
        val tokens = ArgumentTokenizer.tokenize("echo hello\\ world")
        assertEquals(listOf("echo", "hello world"), tokens)
    }

    test("empty input yields empty list") {
        assertTrue(ArgumentTokenizer.tokenize("").isEmpty())
    }
}
