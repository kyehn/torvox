package terminal.emulator

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import terminal.emulator.installer.BootstrapDownloader
import terminal.emulator.installer.BootstrapInstaller
import terminal.emulator.installer.BootstrapOrchestrator
import terminal.emulator.installer.BootstrapProgress
import terminal.emulator.installer.BootstrapProgressCallback
import terminal.emulator.installer.SecondStageRunner
import terminal.emulator.installer.bootstrapDirectories
import terminal.emulator.installer.copyBootstrapArchive
import terminal.emulator.runtime.LogUtil
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.util.TerminalDispatchers
import java.io.File

/**
 * 引导安装的入口与状态：下载/离线安装、URL 编辑防抖取值、结果文案。
 *
 * 从 [TerminalViewModel] 拆出：安装管线只依赖设置仓库与应用上下文，
 * 与会话、选区、字体等职责无关。三条只读流由本类拥有，视图层订阅即可。
 */
internal class BootstrapManager(
    private val settingsRepository: SettingsRepository,
    private val context: Context,
    private val scope: CoroutineScope,
) {
    private val _bootstrapRunning = MutableStateFlow(false)

    /** 引导安装是否在进行中（与用户连点串行化）。 */
    val bootstrapRunning: StateFlow<Boolean> = _bootstrapRunning.asStateFlow()

    private val _bootstrapResult = MutableStateFlow<String?>(null)

    /** 最近一次引导安装的结果文案；null 表示尚无结果。 */
    val bootstrapResult: StateFlow<String?> = _bootstrapResult.asStateFlow()

    private val _bootstrapProgress = MutableStateFlow<BootstrapProgress?>(null)

    /** 最近一次引导安装的进度。 */
    val bootstrapProgress: StateFlow<BootstrapProgress?> = _bootstrapProgress.asStateFlow()

    // 在 UI 线程写入、在 IO 协程读取；volatile 使可见性显式化，
    // 而不依赖隐式的 happens-before 关系。
    @Volatile private var bootstrapUrlEdited = false

    fun setBootstrapUrl(url: String) {
        bootstrapUrlEdited = true
        settingsRepository.recordBootstrapUrlEdit(url)
    }

    /** 把引导结果映射为本地化文案；失败负载是机器可读键（见 [BootstrapOrchestrator]）。 */
    private fun bootstrapOutcomeText(result: Result<String>): String = result.fold(
        onSuccess = { diagnostics ->
            val headline = context.getString(R.string.bootstrap_installed_success)
            if (diagnostics.isEmpty()) {
                headline
            } else {
                context.getString(R.string.bootstrap_postinst_errors) + "\n" + diagnostics
            }
        },
        onFailure = { exception ->
            when (exception.message) {
                BootstrapOrchestrator.ERROR_PRIMARY_USER_REQUIRED ->
                    context.getString(R.string.bootstrap_primary_user_required)

                BootstrapOrchestrator.ERROR_ALREADY_IN_PROGRESS ->
                    context.getString(R.string.bootstrap_already_in_progress)

                BootstrapOrchestrator.ERROR_NO_URL ->
                    context.getString(R.string.bootstrap_no_url)

                // 编排器已把每条失败收敛成只含异常类名的文案（原始消息可能嵌入引导 URL，
                // 故不外泄），这里再取一次类名会把那层收敛整个抹掉：真机上 `file://`
                // 被 SELinux 拒绝时用户只看到「安装失败：Exception」，无从判断该换
                // 预设、换 https 还是改从文件安装。裸异常才退回类名。
                else -> context.getString(
                    R.string.bootstrap_error,
                    exception.message ?: exception.javaClass.simpleName,
                )
            }
        },
    )

    /** 在线与离线安装路径共用的安装器与二阶段执行器配对；目录取自三处入口共用的 [bootstrapDirectories]。 */
    private fun bootstrapComponents(
        onProgress: BootstrapProgressCallback,
    ): Pair<
        BootstrapInstaller,
        SecondStageRunner,
        > {
        val directories = bootstrapDirectories(context)
        return BootstrapInstaller(
            prefixDir = directories.prefix,
            homeDir = directories.home,
            stagingDir = directories.staging,
            onProgress = onProgress,
        ) to SecondStageRunner(
            prefixDir = directories.prefix,
            homeDir = directories.home,
            onProgress = onProgress,
        )
    }

    /**
     * 引导安装共用的守卫与协程骨架：经 [bootstrapRunning] 上的 CAS 串行化并发运行、
     * 重置进度状态，并把失败映射为同一条错误消息。
     * 调用方提供安装主体，它会收到共享的进度回调。
     */
    private fun startBootstrapJob(block: suspend (BootstrapProgressCallback) -> Unit) {
        // CAS 使快速连点安装按钮无法启动两个并发安装。
        if (!_bootstrapRunning.compareAndSet(false, true)) return
        scope.launch(TerminalDispatchers.inputOutput) {
            _bootstrapResult.value = null
            _bootstrapProgress.value = null
            try {
                val onProgress =
                    BootstrapProgressCallback { progress ->
                        _bootstrapProgress.value = progress
                    }
                block(onProgress)
            } catch (exception: Exception) {
                if (exception is CancellationException) throw exception
                LogUtil.error("BootstrapManager", "Bootstrap failed", exception)
                _bootstrapResult.value =
                    context.getString(R.string.bootstrap_error, exception.javaClass.simpleName)
            } finally {
                _bootstrapRunning.value = false
            }
        }
    }

    fun runBootstrap() {
        startBootstrapJob { onProgress ->
            val downloader =
                BootstrapDownloader(
                    context,
                    onProgress = onProgress,
                )
            val (installer, secondStage) = bootstrapComponents(onProgress)
            val orchestrator =
                BootstrapOrchestrator(
                    downloader,
                    installer,
                    secondStage,
                    onProgress = onProgress,
                )
            // 直接读取防抖后的值：DataStore 写入有防抖
            // （terminal.emulator.settings.SettingsRepository.DEBOUNCE_MILLIS），
            // 故用户在输入后立刻点安装时 first() 仍可能返回旧 URL。此前此处硬写
            // 「500ms」而实际是 300ms，改动后就会漂移，故改为引用常量。
            // 已编辑的输入框（即使被清空）优先于已存储的 URL。
            val edited = settingsRepository.latestBootstrapUrlEdit()
            val url =
                if (bootstrapUrlEdited && edited != null) {
                    edited
                } else {
                    settingsRepository.bootstrapUrl.first()
                }
            val result = orchestrator.ensureBootstrap(url)
            _bootstrapResult.value = bootstrapOutcomeText(result)
        }
    }

    /**
     * 从 SAF URI 离线安装引导。用户经 `ActivityResultContracts.OpenDocument` 选择 .zip 文件；
     * 其内容被复制到缓存文件，再送入与在线路径相同的安装器管线
     * （installer.install → secondStage.run）。无需网络；已下载的引导 URL 被忽略。
     */
    fun installOffline(uri: Uri) {
        startBootstrapJob { onProgress ->
            val (installer, secondStage) = bootstrapComponents(onProgress)
            // 把 SAF URI 内容复制到临时缓存文件——安装器需要一个 File
            // （它会对 zip 做哈希与流式读取）。缓存目录总是可写，
            // 且会在系统压力下被清理。
            _bootstrapProgress.value = BootstrapProgress.Downloading(0, 0)
            val cacheFile = File(context.cacheDir, "offline-bootstrap.zip")
            try {
                context.contentResolver.openInputStream(uri)?.use { input ->
                    cacheFile.outputStream().use { output ->
                        copyBootstrapArchive(
                            input = input,
                            output = output,
                            onCopied = { copied ->
                                _bootstrapProgress.value =
                                    BootstrapProgress.Downloading(copied, 0)
                            },
                        )
                    }
                } ?: throw java.io.IOException("Failed to open bootstrap file")
                val result = installer.install(cacheFile)
                if (result.isSuccess) {
                    val secondResult = secondStage.run()
                    if (!secondResult.success) {
                        val failureDiagnostics = secondResult.errors.take(3).joinToString("\n") { "- $it" }
                        _bootstrapResult.value =
                            context.getString(
                                R.string.bootstrap_error,
                                failureDiagnostics.ifEmpty { "Postinst failed" },
                            )
                    } else {
                        _bootstrapProgress.value = BootstrapProgress.Complete
                        val headline = context.getString(R.string.bootstrap_installed_from_file)
                        _bootstrapResult.value = headline
                    }
                } else {
                    _bootstrapResult.value =
                        context.getString(
                            R.string.bootstrap_error,
                            result.exceptionOrNull()?.javaClass?.simpleName,
                        )
                }
            } finally {
                // 中转包无论成败都必须删：留在 cacheDir 的是数百 MB 的引导归档，
                // 复制中途失败留下的半截内容同样占着空间。
                cacheFile.delete()
            }
        }
    }
}
