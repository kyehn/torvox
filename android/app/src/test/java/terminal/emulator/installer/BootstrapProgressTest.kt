package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * BootstrapProgress.overallProgress — the settings-screen install bar.
 * Invariants pinned here: monotonic stage bands (download → extract →
 * symlinks → post-install → complete), never-over-1.0 progress, and
 * division-by-zero safety when totals are unknown (0).
 */
class BootstrapProgressTest {
    private val tolerance = 0.001f

    @Test
    fun `download scales linearly to 85 percent`() {
        assertEquals(0f, BootstrapProgress.Downloading(0, 100).overallProgress(), tolerance)
        assertEquals(0.425f, BootstrapProgress.Downloading(50, 100).overallProgress(), tolerance)
        assertEquals(0.85f, BootstrapProgress.Downloading(100, 100).overallProgress(), tolerance)
    }

    @Test
    fun `download with unknown length stays at zero`() {
        assertEquals(0f, BootstrapProgress.Downloading(42, 0).overallProgress(), tolerance)
    }

    @Test
    fun `extract starts at the download cap`() {
        val empty = BootstrapProgress.Extracting(0, 100).overallProgress()
        assertTrue("first extract step must not regress the bar", empty >= 0.85f)
        assertEquals(0.85f, empty, tolerance)
    }

    @Test
    fun `extract caps at 97 percent`() {
        assertEquals(0.97f, BootstrapProgress.Extracting(100, 100).overallProgress(), tolerance)
        assertEquals(0.91f, BootstrapProgress.Extracting(50, 100).overallProgress(), tolerance)
    }

    @Test
    fun `extract with unknown totals stays inside band`() {
        val progress = BootstrapProgress.Extracting(3, 0).overallProgress()
        assertTrue("unknown totals must not leave the extract band", progress in 0.85f..0.97f)
    }

    @Test
    fun `creating symlinks pins at 99 percent`() {
        assertEquals(0.99f, BootstrapProgress.CreatingSymlinks.overallProgress(), tolerance)
    }

    @Test
    fun `post install interpolates between 99 and 100 percent`() {
        assertEquals(0.99f, BootstrapProgress.RunningPostInstall(0, 10).overallProgress(), tolerance)
        assertEquals(0.995f, BootstrapProgress.RunningPostInstall(5, 10).overallProgress(), tolerance)
        assertEquals(1f, BootstrapProgress.RunningPostInstall(10, 10).overallProgress(), tolerance)
    }

    @Test
    fun `post install with unknown totals stays inside band`() {
        val progress = BootstrapProgress.RunningPostInstall(1, 0).overallProgress()
        assertTrue("unknown totals must stay in 0.99..1.0", progress in 0.99f..1f)
    }

    @Test
    fun `complete is exactly one`() {
        assertEquals(1f, BootstrapProgress.Complete.overallProgress(), tolerance)
    }

    @Test
    fun `error resets to zero`() {
        assertEquals(0f, BootstrapProgress.Error("boom").overallProgress(), tolerance)
    }
}
