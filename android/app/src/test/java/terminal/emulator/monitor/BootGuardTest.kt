package terminal.emulator.monitor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * BootGuard crash-loop detection: N exits inside the reset window disable
 * the auto-kill path; a healthy boot re-enables it. Uses a temp dir for
 * the counter file and real wall-clock time (the 10 min window is far
 * longer than any test run).
 */
@RunWith(RobolectricTestRunner::class)
class BootGuardTest {

    private lateinit var logDir: File

    @Before
    fun setUp() {
        logDir = java.nio.file.Files.createTempDirectory("bootguard").toFile()
        BootGuard.autoKillEnabled = true
    }

    @Test
    fun `fewer than max exits keeps auto kill enabled`() {
        val guard = BootGuard(logDir)
        guard.recordExit()
        guard.check()
        assertTrue("one exit is not a boot loop", BootGuard.autoKillEnabled)
    }

    @Test
    fun `max exits inside the window disable auto kill`() {
        val guard = BootGuard(logDir)
        repeat(BootGuard.MAX_EXITS) { guard.recordExit() }
        guard.check()
        assertFalse("repeated exits must disable auto kill", BootGuard.autoKillEnabled)
    }

    @Test
    fun `mark healthy resets the counter and re-enables auto kill`() {
        val guard = BootGuard(logDir)
        repeat(BootGuard.MAX_EXITS) { guard.recordExit() }
        guard.check()
        assertFalse(BootGuard.autoKillEnabled)
        guard.markHealthy()
        guard.check()
        assertTrue("healthy boot must re-enable auto kill", BootGuard.autoKillEnabled)
    }

    @Test
    fun `rotate logs keeps only the newest per prefix`() {
        val guard = BootGuard(logDir)
        repeat(12) { index ->
            val file = File(logDir, "anr_$index.txt")
            file.writeText("trace $index")
            // Stagger timestamps so ordering is deterministic.
            file.setLastModified(1_000_000L + index * 1_000L)
        }
        guard.rotateLogs(maxFilesPerType = 10)
        val remaining = logDir.listFiles { f -> f.name.startsWith("anr_") }!!.toList()
        assertEquals("oldest entries must be rotated away", 10, remaining.size)
        val names = remaining.map { it.name }.sorted()
        assertFalse("the two oldest files are gone", names.contains("anr_0.txt"))
        assertFalse(names.contains("anr_1.txt"))
        assertTrue("newest entries survive", names.contains("anr_11.txt"))
    }

    @Test
    fun `rotate logs is a no-op below the limit`() {
        val guard = BootGuard(logDir)
        repeat(5) { index ->
            val file = File(logDir, "fatal_$index.txt")
            file.writeText("trace $index")
            file.setLastModified(1_000_000L + index * 1_000L)
        }
        guard.rotateLogs(maxFilesPerType = 10)
        assertEquals(5, logDir.listFiles { f -> f.name.startsWith("fatal_") }!!.size)
    }
}
