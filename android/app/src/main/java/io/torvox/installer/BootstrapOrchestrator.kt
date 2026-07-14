package io.torvox.installer

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

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

    fun getInstallStatus(): Status = if (installer.isInstalled()) {
        Status.INSTALLED
    } else {
        Status.NOT_INSTALLED
    }

    suspend fun ensureBootstrap(bootstrapUrl: String): Result<String> = withContext(Dispatchers.IO) {
        if (installer.isInstalled()) {
            return@withContext Result.success("Bootstrap already installed")
        }
        val resolvedUrl = bootstrapUrl.ifBlank { getDefaultBootstrapUrl() }
        if (resolvedUrl.isBlank()) {
            return@withContext Result.failure(Exception("No bootstrap URL available for this architecture"))
        }
        try {
            onProgress?.onProgress(BootstrapProgress.Downloading(0, 0))
            val arch = detectAbi()
            val zipFile =
                downloader.download(resolvedUrl, arch).getOrElse { exception ->
                    onProgress?.onProgress(BootstrapProgress.Error("Download failed: ${exception.message}"))
                    return@withContext Result.failure(Exception("Download failed: ${exception.message}"))
                }
            installer.install(zipFile).getOrElse { exception ->
                onProgress?.onProgress(BootstrapProgress.Error("Install failed: ${exception.message}"))
                return@withContext Result.failure(Exception("Install failed: ${exception.message}"))
            }
            val secondStageResult = secondStageRunner.run()
            onProgress?.onProgress(BootstrapProgress.CreatingSymlinks)
            val messages = mutableListOf("Bootstrap installed successfully")
            if (secondStageResult.errors.isNotEmpty()) {
                messages.add("${secondStageResult.errors.size} postinst scripts had errors")
            }
            zipFile.delete()
            onProgress?.onProgress(BootstrapProgress.Complete)
            Result.success(messages.joinToString("; "))
        } catch (exception: Exception) {
            val message = "Bootstrap orchestration failed: ${exception.message}"
            onProgress?.onProgress(BootstrapProgress.Error(message))
            Result.failure(Exception(message, exception))
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

    private fun detectAbi(): String = io.torvox.detectArchFromAbi()
}
