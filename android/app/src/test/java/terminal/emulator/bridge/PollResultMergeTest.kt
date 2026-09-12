package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Bridge.PollResult.merge — the frame-event coalescing contract: the
 * FIRST exit in a frame wins (sessionId/exitCode travel together),
 * request lists accumulate (each carries a distinct request_id), and
 * a later clipboard scalar overwrites the earlier one.
 */
class PollResultMergeTest {
    private fun exitResult(sessionId: Long, exitCode: Int) = Bridge.PollResult(
        exit = true,
        exitCode = exitCode,
        exitAliveMs = 1234L,
        sessionId = sessionId,
    )

    // ── exit attribution ──────────────────────────────────────────────

    @Test
    fun `first exit wins over a later non-exit event`() {
        val first = exitResult(sessionId = 7, exitCode = 0)
        val later = Bridge.PollResult(clipboard = "text") // non-exit event
        val merged = first.merge(later)
        assertTrue(merged.exit)
        assertEquals(7, merged.sessionId)
        assertEquals(0, merged.exitCode)
        assertEquals(1234L, merged.exitAliveMs)
    }

    @Test
    fun `first exit wins over a later exit of another session`() {
        val first = exitResult(sessionId = 7, exitCode = 1)
        val later = exitResult(sessionId = 9, exitCode = 2)
        val merged = first.merge(later)
        assertEquals(7, merged.sessionId)
        assertEquals(1, merged.exitCode)
        assertEquals(1234L, merged.exitAliveMs)
    }

    @Test
    fun `exit is sticky when later event has no exit`() {
        val first = exitResult(sessionId = 7, exitCode = 3)
        val merged = first.merge(Bridge.PollResult())
        assertTrue(merged.exit)
        assertEquals(7, merged.sessionId)
        assertEquals(3, merged.exitCode)
    }

    @Test
    fun `no exit anywhere stays inert`() {
        val merged = Bridge.PollResult().merge(Bridge.PollResult())
        assertFalse(merged.exit)
        assertEquals(0, merged.sessionId)
        assertEquals(0, merged.exitCode)
    }

    // ── scalar later-wins fields ──────────────────────────────────────

    @Test
    fun `later clipboard overwrites earlier value`() {
        val first = Bridge.PollResult(clipboard = "a")
        val merged = first.merge(Bridge.PollResult(clipboard = "b"))
        assertEquals("b", merged.clipboard)
    }

    @Test
    fun `null scalar does not clobber an existing value`() {
        val first = Bridge.PollResult(clipboard = "a")
        val merged = first.merge(Bridge.PollResult())
        assertEquals("a", merged.clipboard)
    }

    // ── accumulating lists ────────────────────────────────────────────

    @Test
    fun `clipboard request lists accumulate across frames`() {
        val read1 = Bridge.ClipboardRequest(1, 10, "c")
        val read2 = Bridge.ClipboardRequest(2, 20, "c")
        val merged = Bridge.PollResult(clipboardReads = listOf(read1)).merge(Bridge.PollResult(clipboardReads = listOf(read2)))
        assertEquals(listOf(read1, read2), merged.clipboardReads)
    }

    @Test
    fun `exits list accumulates even though scalar fields pin the first`() {
        val firstExit = Bridge.ExitInfo(7, 1)
        val secondExit = Bridge.ExitInfo(9, 2)
        val first = Bridge.PollResult(exit = true, sessionId = 7, exitCode = 1, exits = listOf(firstExit))
        val later = exitResult(sessionId = 9, exitCode = 2).copy(exits = listOf(secondExit))
        val merged = first.merge(later)
        assertEquals(listOf(firstExit, secondExit), merged.exits)
        // Scalar fields still describe only the first exit.
        assertEquals(7, merged.sessionId)
        assertEquals(1, merged.exitCode)
    }
}
