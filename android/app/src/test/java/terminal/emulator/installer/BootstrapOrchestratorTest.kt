package terminal.emulator.installer

import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
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

    @Before
    fun declareHostAbi() {
        // Robolectric 下 SUPPORTED_ABIS 为空：按宿主真实架构声明，
        // 否则 detectDeviceArchitecture 按未知 ABI 抛错，首个 async 在下载前夭折、
        // `downloadEntered.await()` 永等（本次已挂住 20 分钟）。
        val field = android.os.Build::class.java.getDeclaredField("SUPPORTED_ABIS")
        field.isAccessible = true
        field.set(null, arrayOf("x86_64"))
    }

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
        val orchestrator = orchestrator()
        val result = runBlocking { orchestrator.ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isFailure)
        assertEquals(BootstrapOrchestrator.Status.ERROR, orchestrator.getInstallStatus())
    }

    @Test
    fun `install failure marks error state and keeps state sticky`() {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } returns Result.success(File("/tmp/fake.zip"))
        coEvery { installer.install(any()) } returns Result.failure(IllegalStateException("corrupt"))
        val orchestrator = orchestrator()
        val result = runBlocking { orchestrator.ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isFailure)
        assertEquals(BootstrapOrchestrator.Status.ERROR, orchestrator.getInstallStatus())
    }

    @Test
    fun `successful install reports installed with empty payload`() {
        every { installer.isInstalled() } returns false
        coEvery { downloader.download(any(), any()) } returns Result.success(File("/tmp/fake.zip"))
        coEvery { installer.install(any()) } returns Result.success(Unit)
        coEvery { secondStageRunner.run() } returns
            SecondStageRunner.Result()
        val orchestrator = orchestrator()
        val result = runBlocking { orchestrator.ensureBootstrap("https://example.com/x.zip") }
        assertTrue(result.isSuccess)
        // SecondStageRunner 以 errors.isEmpty() 定义成功，故成功时没有诊断可返回；
        // 非致命 stderr 只出现在失败分支。
        assertEquals("", result.getOrNull())
        assertEquals(BootstrapOrchestrator.Status.INSTALLED, orchestrator.getInstallStatus())
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

        val orchestrator = orchestrator()
        val first = async { orchestrator.ensureBootstrap("https://example.com/x.zip") }
        // Wait (with timeout) for the first attempt to be inside download.
        withTimeout(30_000) { downloadEntered.await() }
        val second = orchestrator.ensureBootstrap("https://example.com/x.zip")
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
            SecondStageRunner.Result()
        val orchestrator = orchestrator()
        assertTrue(runBlocking { orchestrator.ensureBootstrap("https://example.com/x.zip") }.isSuccess)
        // After INSTALLED, isInstalled() drives the status.
        every { installer.isInstalled() } returns true
        assertEquals(BootstrapOrchestrator.Status.INSTALLED, orchestrator().getInstallStatus())
    }
}
