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

/** mksh rc 文件名：写在应用数据目录，作为 [TerminalConfig.mkshrcPath] 注入原生。 */
private const val MKSHRC_FILENAME = ".mkshrc"

/** OSC 0 起始序列：提示符用它上报会话标题（当前目录名）。 */
private const val OSC_TITLE_START = "\u001B]0;"

/** OSC 0 结束序列。 */
private const val OSC_TITLE_END = "\u0007"

/**
 * 拥有会话启动配置：shell 解析、前缀探测、mksh 提示符 rc 与工作目录。
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
     * 写入带 termux 对等提示符的 mksh rc 文件（自愈式）。
     *
     * 根因：无 rc 文件时交互式 mksh 回退到 AOSP `/system/etc/mkshrc` 的
     * 38 列宽提示符 `:/data/.../home $ `。任何长于剩余 ~10 列的命令都会触发 mksh 的
     * 横向滚动重绘（`\r` + 滚动窗口 + `<` 标记 + 退格串），表现为用户报告的乱码回显
     * （"cho …" 碎片与右缘杂散 `<`）。真 Termux 用简短的 `PS1='$ '` 彻底避开。
     *
     * 文件位于应用数据目录而非 `$HOME`：该路径作为 [TerminalConfig.mkshrcPath] 交给原生
     * 并注入为 `$ENV`，交互式 mksh 正是经此读取。
     *
     * mksh 在设置 `$ENV` 时读它，否则交互式读 `~/.mkshrc`；我们先 source 系统 rc
     * （保留其 PATH/别名设置）再覆盖 PS1。引导中的 bash 从不读此文件。
     * 文件不含内容标记时覆写，使陈旧安装自愈。
     *
     * 提示符同时**上报会话标题**（OSC 0，当前目录名）：mksh 自己不发 OSC 0/2
     * （别的 shell 未必自带），没有这层集成，mksh 会话在抽屉里永远只有序号。标题序列
     * 写在提示符里、由 mksh 每次画提示符时发出——mksh 计算提示符宽度时会跳过
     * 转义序列，可见宽度仍是两个字符，不会把命令行顶到折行。
     */
    internal fun ensureMkshPromptRc() {
        val mkshRcFile = File(context.applicationInfo.dataDir, MKSHRC_FILENAME)
        // 新内容独有前缀：含 OSC 7 发射的旧 rc 不匹配，覆盖自愈。
        val contentMarker = "# terminal: termux-parity prompt (see"
        if (mkshRcFile.isFile) {
            try {
                if (mkshRcFile.readText().contains(contentMarker)) return
            } catch (exception: Exception) {
                // 记下读取失败的原因再覆写：只记「写入失败」会把真正的根因（权限/IO）
                // 丢掉，现场只剩一条与病因无关的消息。
                LogUtil.warn("Runtime", "Failed to read $MKSHRC_FILENAME: $exception")
            }
        }
        try {
            mkshRcFile.parentFile?.mkdirs()
            mkshRcFile.writeText(
                "# terminal: termux-parity prompt (see TerminalRuntime.ensureMkshPromptRc)\n" +
                    ". /system/etc/mkshrc\n" +
                    "PS1='$OSC_TITLE_START\${PWD##*/}$OSC_TITLE_END\$ '\n",
            )
        } catch (exception: Exception) {
            LogUtil.warn("Runtime", "Failed to write $MKSHRC_FILENAME: $exception")
        }
    }

    /** [ensureMkshPromptRc] 写入的 mksh rc 文件绝对路径。 */
    internal val mkshrcPath: String
        get() = File(context.applicationInfo.dataDir, MKSHRC_FILENAME).absolutePath

    internal suspend fun buildConfig(failsafeRequested: Boolean, fontSizeTenths: suspend () -> Int): TerminalConfig {
        val configReads = coroutineScope {
            val shellDeferred = async { settingsRepository.shell.first() }
            val fontDeferred = async { fontSizeTenths() }
            ConfigReads(
                shellPath = shellDeferred.await(),
                fontSizeTenths = fontDeferred.await(),
            )
        }
        val shell = resolveShell(configReads.shellPath)
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
            ensureMkshPromptRc()
            return TerminalConfig(
                shell = Shell.SystemDefault,
                fontSizeTenths = configReads.fontSizeTenths,
                home = homeDir,
                workingDirectory = workingDirectory,
                prefix = "",
                mkshrcPath = mkshrcPath,
                scrollbackLines = termuxProperties.configuration.value.properties.terminalTranscriptRows,
            )
        }
        // 默认入口依次探测 bash 与 login（DESIGN.md Shell 节「文件存在即启动，不检查权限」）。
        val prefixShell = findPrefixShell(prefixDir)
        LogUtil.debug(
            "Runtime",
            "prefixShell=$prefixShell prefixDir=$prefixDir",
        )
        val effectivePrefix = if (prefixShell != null) prefixDir else ""
        val effectiveShell = resolveEffectiveShell(prefixDir, prefixShell, shell)
        ensureMkshPromptRc()
        // 启动目录由 `default-working-directory` 决定，未配置时是家目录。
        return TerminalConfig(
            shell = effectiveShell,
            fontSizeTenths = configReads.fontSizeTenths,
            home = homeDir,
            workingDirectory = workingDirectory,
            prefix = effectivePrefix,
            mkshrcPath = mkshrcPath,
            scrollbackLines = termuxProperties.configuration.value.properties.terminalTranscriptRows,
        )
    }

    /** 自定义入口原样透传；默认入口走前缀探测结果，探测失败即系统 shell。 */
    private fun resolveEffectiveShell(prefixDir: String, prefixShell: String?, shell: Shell): Shell {
        if (shell is Shell.Custom) return shell
        val prefixShellChecked = prefixShell ?: return shell
        return Shell.Custom("$prefixDir/$prefixShellChecked")
    }

    private data class ConfigReads(val shellPath: String, val fontSizeTenths: Int)

    /**
     * 默认入口探测：依次尝试 bash 与 login，文件存在即用（DESIGN.md Shell 节），不检查权限。
     */
    private fun findPrefixShell(prefixDir: String): String? = listOf("bin/bash", "bin/login").firstOrNull { candidate ->
        File("$prefixDir/$candidate").isFile
    }

    /** 空即默认入口（DESIGN.md 设置节「未设置时为空」），其余原样透传，不特殊处理。 */
    private fun resolveShell(shellPath: String): Shell = if (shellPath.isEmpty()) {
        Shell.SystemDefault
    } else {
        Shell.Custom(shellPath)
    }
}
