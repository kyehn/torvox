package io.torvox.installer

import org.junit.Assert.assertEquals
import org.junit.Test

class BootstrapProgressTest {
    private val delta = 0.0001f

    @Test
    fun `Downloading progress scales 0 to 85 percent`() {
        assertEquals(0f, BootstrapProgress.Downloading(0, 100).overallProgress(), delta)
        assertEquals(0.425f, BootstrapProgress.Downloading(50, 100).overallProgress(), delta)
        assertEquals(0.85f, BootstrapProgress.Downloading(100, 100).overallProgress(), delta)
    }

    @Test
    fun `Downloading with zero contentLength returns zero`() {
        assertEquals(0f, BootstrapProgress.Downloading(0, 0).overallProgress(), delta)
        assertEquals(0f, BootstrapProgress.Downloading(50, 0).overallProgress(), delta)
    }

    @Test
    fun `Downloading with negative contentLength returns zero`() {
        assertEquals(0f, BootstrapProgress.Downloading(50, -1).overallProgress(), delta)
    }

    @Test
    fun `Downloading step description includes bytes and percentage`() {
        assertEquals(
            "Downloading (0%) (0 B / 100 B)",
            BootstrapProgress.Downloading(0, 100).stepDescription(),
        )
        assertEquals(
            "Downloading (50%) (50 B / 100 B)",
            BootstrapProgress.Downloading(50, 100).stepDescription(),
        )
        assertEquals(
            "Downloading (100%) (100 B / 100 B)",
            BootstrapProgress.Downloading(100, 100).stepDescription(),
        )
    }

    @Test
    fun `Downloading step description without content-length omits total`() {
        assertEquals("Downloading (0 B)", BootstrapProgress.Downloading(0, 0).stepDescription())
        assertEquals("Downloading (0 B)", BootstrapProgress.Downloading(0, -1).stepDescription())
    }

    @Test
    fun `Downloading step description uses MB for large values`() {
        assertEquals(
            "Downloading (50%) (50 MB / 100 MB)",
            BootstrapProgress.Downloading(MB * 50, MB * 100).stepDescription(),
        )
    }

    @Test
    fun `Downloading step description uses KB`() {
        assertEquals(
            "Downloading (50%) (1 KB / 2 KB)",
            BootstrapProgress.Downloading(KB, KB * 2).stepDescription(),
        )
    }

    @Test
    fun `Extracting progress scales from 85 to 100 percent`() {
        assertEquals(0.85f, BootstrapProgress.Extracting(0, 10).overallProgress(), delta)
        assertEquals(0.925f, BootstrapProgress.Extracting(5, 10).overallProgress(), delta)
        assertEquals(1f, BootstrapProgress.Extracting(10, 10).overallProgress(), delta)
    }

    @Test
    fun `Extracting with zero totalEntries returns base progress`() {
        assertEquals(0.85f, BootstrapProgress.Extracting(0, 0).overallProgress(), delta)
        assertEquals(0.85f, BootstrapProgress.Extracting(5, 0).overallProgress(), delta)
    }

    @Test
    fun `Extracting step description shows entry count`() {
        assertEquals(
            "Extracting (0%) (0 / 10)",
            BootstrapProgress.Extracting(0, 10).stepDescription(),
        )
        assertEquals(
            "Extracting (50%) (5 / 10)",
            BootstrapProgress.Extracting(5, 10).stepDescription(),
        )
        assertEquals(
            "Extracting (100%) (10 / 10)",
            BootstrapProgress.Extracting(10, 10).stepDescription(),
        )
    }

    @Test
    fun `RunningPostInstall progress scales from 97 to 99 percent`() {
        assertEquals(0.97f, BootstrapProgress.RunningPostInstall(0, 10).overallProgress(), delta)
        assertEquals(0.98f, BootstrapProgress.RunningPostInstall(5, 10).overallProgress(), delta)
        assertEquals(0.99f, BootstrapProgress.RunningPostInstall(10, 10).overallProgress(), delta)
    }

    @Test
    fun `RunningPostInstall step description`() {
        assertEquals(
            "Running post-install scripts... (0 / 10)",
            BootstrapProgress.RunningPostInstall(0, 10).stepDescription(),
        )
        assertEquals(
            "Running post-install scripts... (5 / 10)",
            BootstrapProgress.RunningPostInstall(5, 10).stepDescription(),
        )
    }

    @Test
    fun `CreatingSymlinks returns fixed values`() {
        assertEquals(0.99f, BootstrapProgress.CreatingSymlinks.overallProgress(), delta)
        assertEquals("Creating symlinks...", BootstrapProgress.CreatingSymlinks.stepDescription())
    }

    @Test
    fun `Complete returns fixed values`() {
        assertEquals(1f, BootstrapProgress.Complete.overallProgress(), delta)
        assertEquals("Bootstrap complete!", BootstrapProgress.Complete.stepDescription())
    }

    @Test
    fun `Error returns zero progress and message`() {
        assertEquals(0f, BootstrapProgress.Error("failed").overallProgress(), delta)
        assertEquals("failed", BootstrapProgress.Error("failed").stepDescription())
    }

    companion object {
        private const val KB = 1024L
        private const val MB = KB * 1024
    }
}
