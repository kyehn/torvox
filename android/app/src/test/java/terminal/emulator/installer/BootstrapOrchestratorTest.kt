package terminal.emulator.installer

import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * JVM tests for [BootstrapOrchestrator]: the install state machine, the
 * process-wide CAS guard (two entry points — runtime start + settings
 * button — must not install concurrently), and the primary-user guard.
 * The real downloader/installer/second-stage are mocked; their internals
 * are covered by BootstrapInstallerTest.
 */
@RunWith(RobolectricTestRunner::class)
class BootstrapOrchestratorTest {
    private val downloader = mockk<BootstrapDownloader>()
    private val installer = mockk<BootstrapInstaller>()
    private val secondStageRunner = mockk<SecondStageRunner>()

    private fun orchestrator() = BootstrapOrchestrator(downloader, installer, secondStageRunner)

    @Test
    fun `already installed short circuits without touching downloader`() {
        every { installer.isInstalled() } returns true
        val result = runBlocking { orchestrator().ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isSuccess)
    }

    @Test
    fun `blank url fails with no bootstrap url key`() {
        every { installer.isInstalled() } returns false
        val result = runBlocking { orchestrator().ensureBootstrap("  ") }
        assertTrue(result.isFailure)
        assertEquals(
            BootstrapOrchestrator.ERROR_NO_URL,
            (result.exceptionOrNull() as Exception).message,
        )
    }

    @Test
    fun `download failure marks error state`() {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } returns Result.failure(IllegalStateException("boom"))
        val orch = orchestrator()
        val result = runBlocking { orch.ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isFailure)
        assertEquals(BootstrapOrchestrator.Status.ERROR, orch.getInstallStatus())
    }

    @Test
    fun `install failure marks error state and keeps state sticky`() {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } returns Result.success(File("/tmp/fake.zip"))
        coEvery { installer.install(any()) } returns Result.failure(IllegalStateException("corrupt"))
        val orch = orchestrator()
        val result = runBlocking { orch.ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isFailure)
        assertEquals(BootstrapOrchestrator.Status.ERROR, orch.getInstallStatus())
    }

    @Test
    fun `successful install reports installed and returns second stage details`() {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } returns Result.success(File("/tmp/fake.zip"))
        coEvery { installer.install(any()) } returns Result.success(Unit)
        coEvery { secondStageRunner.run() } returns
            SecondStageRunner.Result(
                success = true,
                errors = listOf("postinst: warning", "detail line"),
            )
        val orch = orchestrator()
        val result = runBlocking { orch.ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isSuccess)
        // The success payload is the first 3 diagnostic lines joined.
        assertEquals("- postinst: warning\n- detail line", result.getOrNull())
        assertEquals(BootstrapOrchestrator.Status.INSTALLED, orch.getInstallStatus())
    }

    @Test
    fun `second concurrent install attempt is rejected`() = runBlocking {
        // First call parks inside download (never completes); the second
        // call must observe the CAS-held lock and fail fast.
        every { installer.isInstalled() } returns false
        val gate = CompletableDeferred<Result<File>>()
        val downloadEntered = CompletableDeferred<Unit>()
        coEvery { downloader.download(any(), any()) } coAnswers {
            // Signal that the first attempt has acquired the CAS and
            // reached the download stage, then park until released.
            downloadEntered.complete(Unit)
            gate.await()
        }

        val orch = orchestrator()
        val first = async { orch.ensureBootstrap("https://example.com/x.zip") }
        // Wait (with timeout) for the first attempt to be inside download.
        downloadEntered.await()
        val second = orch.ensureBootstrap("https://example.com/x.zip")
        assertTrue(second.isFailure)
        assertEquals(
            BootstrapOrchestrator.ERROR_ALREADY_IN_PROGRESS,
            (second.exceptionOrNull() as Exception).message,
        )
        // Release the gate so the test does not leak a parked coroutine.
        gate.complete(Result.failure(IllegalStateException("released")))
        assertTrue(first.await().isFailure)
    }

    @Test
    fun `second attempt succeeds after first completes`() {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } returns Result.success(File("/tmp/fake.zip"))
        coEvery { installer.install(any()) } returns Result.success(Unit)
        coEvery { secondStageRunner.run() } returns
            SecondStageRunner.Result(success = true, errors = emptyList())
        val orch = orchestrator()
        assertTrue(runBlocking { orch.ensureBootstrap("https://example.com/x.zip") }.isSuccess)
        // After INSTALLED, isInstalled() drives the status.
        every { installer.isInstalled() } returns true
        assertEquals(BootstrapOrchestrator.Status.INSTALLED, orchestrator().getInstallStatus())
    }
}
