package io.term.installer

import io.mockk.coEvery
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.junit4.MockKRule
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

@RunWith(RobolectricTestRunner::class)
class BootstrapOrchestratorTest {
    @get:Rule
    val mockkRule = MockKRule(this)

    @MockK(relaxed = true)
    lateinit var downloader: BootstrapDownloader

    @MockK(relaxed = true)
    lateinit var installer: BootstrapInstaller

    @MockK(relaxed = true)
    lateinit var secondStageRunner: SecondStageRunner

    @Test
    fun `already installed returns success immediately`() = runBlocking {
        every { installer.isInstalled() } returns true
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner)
        val result = orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertTrue(result.isSuccess)
        assertEquals("Bootstrap already installed", result.getOrNull())
    }

    @Test
    fun `download failure emits Error progress and returns failure`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.failure(Exception("Network error")) }

        val captured = mutableListOf<BootstrapProgress>()
        val onProgress = BootstrapProgressCallback { captured.add(it) }
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner, onProgress)

        val result = orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("Download failed") == true)
        assertTrue(captured.isNotEmpty())
        assertTrue(captured.any { it is BootstrapProgress.Downloading })
        assertTrue(captured.any { it is BootstrapProgress.Error })
    }

    @Test
    fun `install failure emits Error progress and returns failure`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.success(File("/tmp/test.zip")) }
        coEvery { installer.install(any()) } coAnswers { Result.failure(Exception("Install error")) }

        val captured = mutableListOf<BootstrapProgress>()
        val onProgress = BootstrapProgressCallback { captured.add(it) }
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner, onProgress)

        val result = orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("Install failed") == true)
        assertTrue(captured.any { it is BootstrapProgress.Error })
    }

    @Test
    fun `full success reports all phases and Complete`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.success(File("/tmp/test.zip")) }
        coEvery { installer.install(any()) } coAnswers { Result.success(Unit) }
        coEvery { secondStageRunner.run() } coAnswers { SecondStageRunner.Result(true) }

        val captured = mutableListOf<BootstrapProgress>()
        val onProgress = BootstrapProgressCallback { captured.add(it) }
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner, onProgress)

        val result = orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertTrue(result.isSuccess)

        val names = captured.joinToString { it::class.simpleName ?: "?" }
        assertTrue(
            "Expected 3 events but got ${captured.size}: $names",
            captured.size == 3,
        )
        assertTrue(captured[0] is BootstrapProgress.Downloading)
        assertTrue(captured[1] is BootstrapProgress.CreatingSymlinks)
        assertTrue(captured[2] is BootstrapProgress.Complete)
        assertEquals("Bootstrap installed successfully", result.getOrNull())
    }

    @Test
    fun `postinstall errors return success with error message`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.success(File("/tmp/test.zip")) }
        coEvery { installer.install(any()) } coAnswers { Result.success(Unit) }
        coEvery { secondStageRunner.run() } coAnswers { SecondStageRunner.Result(false, listOf("test-package.postinst failed")) }

        val captured = mutableListOf<BootstrapProgress>()
        val onProgress = BootstrapProgressCallback { captured.add(it) }
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner, onProgress)

        val result = orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertTrue(result.isSuccess)
        val message = result.getOrNull() ?: ""
        assertEquals("Bootstrap installed successfully; 1 postinst scripts had errors", message)
        assertTrue(captured.any { it is BootstrapProgress.Complete })
    }

    @Test
    fun `progress callback fires at phase boundaries`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.success(File("/tmp/test.zip")) }
        coEvery { installer.install(any()) } coAnswers { Result.success(Unit) }
        coEvery { secondStageRunner.run() } coAnswers { SecondStageRunner.Result(true) }

        val captured = mutableListOf<BootstrapProgress>()
        val onProgress = BootstrapProgressCallback { captured.add(it) }
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner, onProgress)

        orchestrator.ensureBootstrap("https://example.com/test.zip")
        val names = captured.joinToString { it::class.simpleName ?: "?" }
        assertTrue(
            "Expected 3 events but got ${captured.size}: $names",
            captured.size == 3,
        )
        assertEquals(BootstrapProgress.Downloading::class, captured[0]::class)
        assertEquals(BootstrapProgress.CreatingSymlinks::class, captured[1]::class)
        assertEquals(BootstrapProgress.Complete::class, captured[2]::class)
    }

    @Test
    fun `getInstallStatus reports NOT_INSTALLED before any run`() {
        every { installer.isInstalled() } returns false
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner)
        assertEquals(BootstrapOrchestrator.Status.NOT_INSTALLED, orchestrator.getInstallStatus())
    }

    @Test
    fun `getInstallStatus reports INSTALLED when already installed`() {
        every { installer.isInstalled() } returns true
        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner)
        assertEquals(BootstrapOrchestrator.Status.INSTALLED, orchestrator.getInstallStatus())
    }

    @Test
    fun `getInstallStatus reports INSTALLED after successful install`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.success(File("/tmp/test.zip")) }
        coEvery { installer.install(any()) } coAnswers { Result.success(Unit) }
        coEvery { secondStageRunner.run() } coAnswers { SecondStageRunner.Result(true) }

        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner)
        orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertEquals(BootstrapOrchestrator.Status.INSTALLED, orchestrator.getInstallStatus())
    }

    @Test
    fun `getInstallStatus reports ERROR after failed install`() = runBlocking {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } coAnswers { Result.success(File("/tmp/test.zip")) }
        coEvery { installer.install(any()) } coAnswers { Result.failure(Exception("Install error")) }

        val orchestrator = BootstrapOrchestrator(downloader, installer, secondStageRunner)
        orchestrator.ensureBootstrap("https://example.com/test.zip")
        assertEquals(BootstrapOrchestrator.Status.ERROR, orchestrator.getInstallStatus())
    }
}
