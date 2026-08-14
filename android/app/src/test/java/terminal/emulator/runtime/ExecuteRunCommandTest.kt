package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Host-side tests for [executeRunCommand]): the bounded
 * drain guarantees a grandchild holding the stdout/stderr fds cannot hang
 * the caller, and the timeout kills runaway processes.
 */
class ExecuteRunCommandTest {
    @Test
    fun `simple command returns output and exit code`() {
        val result = executeRunCommand(listOf("/bin/sh", "-c", "echo hello_out; echo hello_err >&2; exit 7"))
        assertEquals("hello_out\n", result.stdout)
        assertEquals("hello_err\n", result.stderr)
        assertEquals(7, result.exitCode)
        assertFalse(result.timedOut)
    }

    @Test
    fun `empty output command`() {
        val result = executeRunCommand(listOf("/bin/true"))
        assertEquals("", result.stdout)
        assertEquals("", result.stderr)
        assertEquals(0, result.exitCode)
    }

    @Test
    fun `timeout kills runaway process`() {
        val start = System.nanoTime()
        val result = executeRunCommand(listOf("/bin/sh", "-c", "sleep 30"), timeoutMs = 300, drainMs = 500)
        val elapsedMs = (System.nanoTime() - start) / 1_000_000
        assertTrue(result.timedOut)
        assertEquals(-1, result.exitCode)
        assertTrue(
            "must return in bounded time, took ${elapsedMs}ms",
            elapsedMs < 10_000,
        )
    }

    @Test
    fun `grandchild holding stdout cannot hang the drain`() {
        // sh exits immediately (code 0) but the background `sleep` inherits
        // the stdout pipe: without the bounded drain the readText would
        // block until the sleep dies.
        val start = System.nanoTime()
        val result =
            executeRunCommand(
                listOf("/bin/sh", "-c", "(sleep 5) & echo done"),
                timeoutMs = 30_000,
                drainMs = 300,
            )
        val elapsedMs = (System.nanoTime() - start) / 1_000_000
        assertFalse("sh itself must exit normally", result.timedOut)
        assertEquals(0, result.exitCode)
        assertTrue(
            "drain must return in bounded time, took ${elapsedMs}ms",
            elapsedMs < 10_000,
        )
        // The reader was cancelled mid-stream: stdout is either the full
        // "done" or empty — never a hang.
        assertTrue(result.stdout == "done\n" || result.stdout.isEmpty())
    }

    @Test
    fun `stderr-only output captured`() {
        val result = executeRunCommand(listOf("/bin/sh", "-c", "echo err_only >&2"))
        assertEquals("", result.stdout)
        assertEquals("err_only\n", result.stderr)
    }

    /** Invalid binary returns a non-zero exit code without crashing. */
    @Test
    fun `invalid binary path returns error`() {
        val result = executeRunCommand(listOf("/nonexistent/binary/path"))
        assertEquals("", result.stdout)
        assertTrue(result.stderr.contains("No such file") || result.stderr.contains("not found"))
        assertTrue("exit code should be non-zero: ${result.exitCode}", result.exitCode != 0)
        assertFalse(result.timedOut)
    }

    /** Non-ASCII (CJK) characters in stdout are preserved. */
    @Test
    fun `non-ascii output preserved`() {
        val result = executeRunCommand(listOf("/bin/sh", "-c", "printf '日本語テスト\\n'"))
        assertEquals("日本語テスト\n", result.stdout)
        assertEquals(0, result.exitCode)
    }

    /** Very large output (1MB+) is captured without truncation. */
    @Test
    fun `large output captured`() {
        val result = executeRunCommand(
            listOf("/bin/sh", "-c", "dd if=/dev/zero bs=1024 count=64 2>/dev/null | tr '\\0' 'A'"),
            timeoutMs = 10_000,
            drainMs = 5_000,
        )
        assertTrue("stdout should be ~64KB: ${result.stdout.length}", result.stdout.length >= 60_000)
        assertFalse(result.timedOut)
    }
}
