package terminal.emulator.monitor

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * BootGuard crash-loop detection: N exits inside the reset window disable
 * the auto-kill path; a healthy boot re-enables it. Uses a temp dir for
 * the counter file and real wall-clock time (the 10 min window is far
 * longer than any test run).
 */
@RunWith(RobolectricTestRunner::class)
class BootGuardTest {

    private lateinit var stateDir: java.io.File

    @Before
    fun setUp() {
        stateDir = java.nio.file.Files.createTempDirectory("bootguard").toFile()
        BootGuard.autoKillEnabled = true
    }

    @Test
    fun `fewer than max exits keeps auto kill enabled`() {
        val guard = BootGuard(stateDir)
        guard.recordExit()
        guard.check()
        assertTrue("one exit is not a boot loop", BootGuard.autoKillEnabled)
    }

    @Test
    fun `max exits inside the window disable auto kill`() {
        val guard = BootGuard(stateDir)
        repeat(BootGuard.MAX_EXITS) { guard.recordExit() }
        guard.check()
        assertFalse("repeated exits must disable auto kill", BootGuard.autoKillEnabled)
    }

    @Test
    fun `mark healthy resets the counter and re-enables auto kill`() {
        val guard = BootGuard(stateDir)
        repeat(BootGuard.MAX_EXITS) { guard.recordExit() }
        guard.check()
        assertFalse(BootGuard.autoKillEnabled)
        guard.markHealthy()
        guard.check()
        assertTrue("healthy boot must re-enable auto kill", BootGuard.autoKillEnabled)
    }
}
