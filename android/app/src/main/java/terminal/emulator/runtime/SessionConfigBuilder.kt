package terminal.emulator.runtime

import android.content.Context
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxPropertiesRepository
import java.io.File

/**
 * 拥有会话启动配置：shell 解析、前缀探测与工作目录。
 *
 * 会话生命周期（创建、切换、关闭）在 [TerminalRuntime]；「这个会话用什么 shell、
 * 在哪个目录、以多大字号启动」由本类回答，两者互不依赖对方的流程细节。
 */
internal class SessionConfigBuilder(
    private val context: Context,
    private val settingsRepository: SettingsRepository,
    private val termuxProperties: TermuxPropertiesRepository,
) {
    /**
     * [shell] 只用于长按未激活入口新建会话：它绕过设置里的激活项，
     * 但不改变激活状态，也不影响后续会话。
     */
    internal suspend fun buildConfig(
        failsafeRequested: Boolean,
        fontSizeTenths: suspend () -> Int,
        shell: String? = null,
    ): TerminalConfig {
        val configReads = coroutineScope {
            val shellDeferred = async { settingsRepository.shell.first() }
            val fontDeferred = async { fontSizeTenths() }
            ConfigReads(
                shellPath = shellDeferred.await(),
                fontSizeTenths = fontDeferred.await(),
            )
        }
        val configuredShell = resolveShell(shell ?: configReads.shellPath)
        val prefixDir = File(context.filesDir, "usr").absolutePath
        val homeDir =
            File(context.filesDir, "home")
                .apply {
                    if (!exists() && !mkdirs()) {
                        LogUtil.warn("Runtime", "Failed to create home directory: $this")
                    }
                }
                .absolutePath
        // `default-working-directory`：termux 语义是「空值 = 家目录，相对路径相对家目录，
        // `~` 展开为家目录」。不存在或不可读时由 resolveWorkingDirectory 回退家目录。
        val workingDirectory =
            resolveWorkingDirectory(
                termuxProperties.configuration.value.properties.defaultWorkingDirectory,
                homeDir,
            )
        // Failsafe（termux 应用快捷方式 “New session (Failsafe)”）：完全绕开引导程序——
        // 系统 shell、系统 PATH、无 PREFIX——避免引导程序损坏后终端彻底不可用
        // （对应 termux-app TermuxSession.java:95-113 的 isFailsafe 路径）。
        // 只读不消费：消费由建会话成功后的 [consumeFailsafeRequest] 完成——
        // 建会话失败时应急请求仍在，下一次尝试仍应是应急会话。
        if (failsafeRequested) {
            return TerminalConfig(
                shell = Shell.SystemDefault,
                fontSizeTenths = configReads.fontSizeTenths,
                home = homeDir,
                workingDirectory = workingDirectory,
                prefix = "",
                scrollbackLines = termuxProperties.configuration.value.properties.terminalTranscriptRows,
            )
        }
        // 默认入口依次探测（DESIGN.md Shell 节「文件存在即启动，不检查权限」）。
        val prefixShell = defaultShellEntry(prefixDir)
        LogUtil.debug(
            "Runtime",
            "prefixShell=$prefixShell prefixDir=$prefixDir",
        )
        val effectivePrefix = if (prefixShell != null) prefixDir else ""
        val effectiveShell = resolveEffectiveShell(prefixDir, prefixShell, configuredShell)
        // 启动目录由 `default-working-directory` 决定，未配置时是家目录。
        return TerminalConfig(
            shell = effectiveShell,
            fontSizeTenths = configReads.fontSizeTenths,
            home = homeDir,
            workingDirectory = workingDirectory,
            prefix = effectivePrefix,
            scrollbackLines = termuxProperties.configuration.value.properties.terminalTranscriptRows,
        )
    }

    /** 自定义入口原样透传；默认入口走前缀探测结果，探测失败即系统 shell。 */
    private fun resolveEffectiveShell(prefixDir: String, prefixShell: String?, shell: Shell): Shell {
        if (shell is Shell.Custom) return shell
        val prefixShellChecked = prefixShell ?: return shell
        val loginArgument = if (prefixShellChecked in LOGIN_SHELL_CANDIDATES) " -l" else ""
        return Shell.Custom("$prefixDir/$prefixShellChecked$loginArgument")
    }

    private companion object {
        /** 默认入口探测顺序与 DESIGN「Shell」节一致，命中者按登录 shell 启动。 */
        val LOGIN_SHELL_CANDIDATES = listOf("bin/login", "bin/bash", "bin/zsh", "bin/fish", "bin/sh")

        /** 文件存在即用，不检查权限（DESIGN「Shell」节）。 */
        fun defaultShellEntry(prefixDir: String): String? =
            LOGIN_SHELL_CANDIDATES.firstOrNull { candidate -> File("$prefixDir/$candidate").isFile }
    }

    private data class ConfigReads(val shellPath: String, val fontSizeTenths: Int)

    /** 空即默认入口（DESIGN.md 设置节「未设置时为空」），其余原样透传，不特殊处理。 */
    private fun resolveShell(shellPath: String): Shell = if (shellPath.isEmpty()) {
        Shell.SystemDefault
    } else {
        Shell.Custom(shellPath)
    }
}
