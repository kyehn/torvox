package terminal.emulator.installer

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.concurrent.atomic.AtomicReference

private const val BOOTSTRAP_BASE_URL = "https://github.com/termux/termux-packages/releases/download/bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-"

class BootstrapOrchestrator(
    private val downloader: BootstrapDownloader,
    private val installer: BootstrapInstaller,
    private val secondStageRunner: SecondStageRunner,
    private val onProgress: BootstrapProgressCallback? = null,
) {
    enum class Status {
        NOT_INSTALLED,
        INSTALLED,
        INSTALLING,
        ERROR,
    }

    private val state = AtomicReference(Status.NOT_INSTALLED)

    // Process-wide mutex: TerminalRuntime.start() and the settings
    // bootstrap button each construct their own BootstrapOrchestrator
    // instance, so an instance field would not stop them from
    // downloading/installing into the same staging directory
    // concurrently (corrupting each other's files).
    companion object {
        private val processLock = Any()
        private val processInstalling = java.util.concurrent.atomic.AtomicBoolean(false)
    }

    fun getInstallStatus(): Status = if (installer.isInstalled()) {
        Status.INSTALLED
    } else {
        state.get()
    }

    suspend fun ensureBootstrap(bootstrapUrl: String): Result<String> = withContext(Dispatchers.IO) {
        if (installer.isInstalled()) {
            return@withContext Result.success("Bootstrap already installed")
        }
        // Mutex via CAS: two concurrent entry points (runtime start and
        // settings bootstrap button) must not both download/install into
        // the same staging directory — they would delete each other's
        // in-progress files and corrupt the install. Retry is allowed
        // from both NOT_INSTALLED and ERROR.
        if (!processInstalling.compareAndSet(false, true)) {
            return@withContext Result.failure(Exception("Bootstrap installation already in progress"))
        }
        try {
            ensureBootstrapLocked(bootstrapUrl)
        } finally {
            processInstalling.set(false)
        }
    }

    private suspend fun ensureBootstrapLocked(bootstrapUrl: String): Result<String> {
        if (installer.isInstalled()) {
            return Result.success("Bootstrap already installed")
        }
        synchronized(processLock) {
            state.set(Status.INSTALLING)
        }
        val resolvedUrl = bootstrapUrl.ifBlank { getDefaultBootstrapUrl() }
        if (resolvedUrl.isBlank()) {
            state.set(Status.ERROR)
            return Result.failure(Exception("No bootstrap URL available for this architecture"))
        }
        try {
            onProgress?.onProgress(BootstrapProgress.Downloading(0, 0))
            val arch = detectAbi()
            val zipFile =
                downloader.download(resolvedUrl, arch).getOrElse { exception ->
                    onProgress?.onProgress(BootstrapProgress.Error("Download failed: ${exception.javaClass.simpleName}"))
                    state.set(Status.ERROR)
                    return Result.failure(Exception("Download failed: ${exception.javaClass.simpleName}"))
                }
            try {
                installer.install(zipFile).getOrElse { exception ->
                    onProgress?.onProgress(BootstrapProgress.Error("Install failed: ${exception.javaClass.simpleName}"))
                    state.set(Status.ERROR)
                    return Result.failure(Exception("Install failed: ${exception.javaClass.simpleName}"))
                }
                val secondStageResult = secondStageRunner.run()
                // CreatingSymlinks progress is emitted inside
                // BootstrapInstaller.install() where the symlinks are
                // actually created; a duplicate here would be out of order
                // (round-107).
                val messages = mutableListOf("Bootstrap installed successfully")
                if (secondStageResult.errors.isNotEmpty()) {
                    messages.add("${secondStageResult.errors.size} postinst scripts had errors")
                }
                onProgress?.onProgress(BootstrapProgress.Complete)
                state.set(Status.INSTALLED)
                return Result.success(messages.joinToString("; "))
            } finally {
                // Always drop the downloaded archive, including failure
                // paths: a 150 MB file left in cacheDir is never reused.
                zipFile.delete()
            }
        } catch (exception: Exception) {
            // Class name only: the underlying failure may embed the
            // bootstrap URL (round-105).
            val message = "Bootstrap orchestration failed: ${exception.javaClass.simpleName}"
            onProgress?.onProgress(BootstrapProgress.Error(message))
            state.set(Status.ERROR)
            // No cause chain: the original exception may embed the bootstrap
            // URL/host (round-106); consumers only see the redacted message.
            return Result.failure(Exception(message))
        }
    }

    private fun getDefaultBootstrapUrl(): String {
        val arch = detectAbi()
        return when (arch) {
            "aarch64" -> "${BOOTSTRAP_BASE_URL}aarch64.zip"
            "arm" -> "${BOOTSTRAP_BASE_URL}arm.zip"
            "x86_64" -> "${BOOTSTRAP_BASE_URL}x86_64.zip"
            "i686" -> "${BOOTSTRAP_BASE_URL}i686.zip"
            else -> ""
        }
    }

    private fun detectAbi(): String = terminal.emulator.detectArchFromAbi()
}
