package terminal.emulator.installer

import kotlinx.coroutines.withContext
import terminal.emulator.util.TerminalDispatchers
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

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

    // 进程级互斥：TerminalRuntime.start() 与设置里的引导按钮各自构造
    // 自己的 BootstrapOrchestrator 实例，故实例字段无法阻止它们
    // 并发地向同一 staging 目录下载/安装（互相破坏文件）——由
    // [processInstalling] 的 CAS 独占，安装全程再无第二处共享状态。
    companion object {
        /** 由 UI 层本地化展示的机器可读失败键。 */
        const val ERROR_PRIMARY_USER_REQUIRED = "primary_user_required"
        const val ERROR_ALREADY_IN_PROGRESS = "already_in_progress"
        const val ERROR_NO_URL = "no_bootstrap_url"
        const val ERROR_CANCELLED = "cancelled"

        private val processInstalling = AtomicBoolean(false)
    }

    fun getInstallStatus(): Status = if (installer.isInstalled()) {
        Status.INSTALLED
    } else {
        state.get()
    }

    suspend fun ensureBootstrap(bootstrapUrl: String): Result<String> = withContext(TerminalDispatchers.inputOutput) {
        if (installer.isInstalled()) {
            return@withContext Result.success("")
        }
        // 多用户防护：引导写入的目录仅主用户（userId 0）可访问。
        // 次用户共享 UID 命名空间但无法访问主用户的私有目录，
        // 会导致静默的 SELinux 拒绝与 exec 失败。
        // Android UID 以 uid / 100_000 编码 userId（稳定的 ABI 约定）。
        val userId = android.os.Process.myUid() / 100_000
        if (userId != 0) {
            return@withContext Result.failure(Exception(ERROR_PRIMARY_USER_REQUIRED))
        }
        // 经 CAS 的互斥：两个并发入口（运行期启动与设置引导按钮）
        // 绝不能同时向同一 staging 目录下载/安装
        // ——它们会删除彼此进行中的文件并损坏安装。
        // NOT_INSTALLED 与 ERROR 两种状态都允许重试。
        if (!processInstalling.compareAndSet(false, true)) {
            return@withContext Result.failure(Exception(ERROR_ALREADY_IN_PROGRESS))
        }
        try {
            ensureBootstrapLocked(bootstrapUrl)
        } finally {
            processInstalling.set(false)
        }
    }

    private suspend fun ensureBootstrapLocked(bootstrapUrl: String): Result<String> {
        if (installer.isInstalled()) {
            return Result.success("")
        }
        state.set(Status.INSTALLING)
        val resolvedUrl = bootstrapUrl
        if (resolvedUrl.isBlank()) {
            state.set(Status.ERROR)
            return Result.failure(Exception(ERROR_NO_URL))
        }
        try {
            onProgress?.onProgress(BootstrapProgress.Downloading(0, 0))
            val arch = detectAbi()
            val zipFile =
                downloader.download(resolvedUrl, arch).getOrElse { exception ->
                    onProgress?.onProgress(BootstrapProgress.Error(exception.javaClass.simpleName))
                    state.set(Status.ERROR)
                    return Result.failure(Exception("Download failed: ${exception.javaClass.simpleName}"))
                }
            try {
                installer.install(zipFile).getOrElse { exception ->
                    onProgress?.onProgress(BootstrapProgress.Error(exception.javaClass.simpleName))
                    state.set(Status.ERROR)
                    return Result.failure(Exception("Install failed: ${exception.javaClass.simpleName}"))
                }
                val secondStageResult = secondStageRunner.run()
                if (!secondStageResult.success) {
                    onProgress?.onProgress(BootstrapProgress.Error("Postinst failed"))
                    state.set(Status.ERROR)
                    val failureDetails = secondStageResult.errors.take(3).joinToString("\n") { "- $it" }
                    return Result.failure(Exception(failureDetails.ifEmpty { "Postinst failed" }))
                }
                // CreatingSymlinks 进度在 BootstrapInstaller.install() 内部
                // 符号链接真正创建时发出；在此重复会乱序。
                onProgress?.onProgress(BootstrapProgress.Complete)
                state.set(Status.INSTALLED)
                // 成功负载只携带安装后诊断信息（最多 3 段 stderr 摘录，
                // 每段截断到 400 字符）；成功标题由 UI 层本地化。
                val details = secondStageResult.errors.take(3).joinToString("\n") { "- $it" }
                return Result.success(details)
            } finally {
                // 始终删除下载的归档，失败路径亦然：
                // 留在 cacheDir 的 150 MB 文件永不复用。
                zipFile.delete()
            }
        } catch (exception: Exception) {
            // 只取异常类名：底层失败可能嵌入引导 URL。
            val message = "Bootstrap orchestration failed: ${exception.javaClass.simpleName}"
            onProgress?.onProgress(BootstrapProgress.Error(message))
            state.set(Status.ERROR)
            // 不带 cause 链：原异常可能嵌入引导 URL/主机；消费者只看到已脱敏的消息。
            return Result.failure(Exception(message))
        }
    }

    private fun detectAbi(): String = terminal.emulator.detectArchFromAbi()
}
