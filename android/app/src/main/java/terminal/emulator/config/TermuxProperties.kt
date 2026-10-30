package terminal.emulator.config

import terminal.emulator.bridge.CursorStyle
import terminal.emulator.runtime.LogUtil
import java.io.File
import java.io.InputStreamReader
import java.nio.charset.StandardCharsets
import java.util.Properties

/** `termux.properties` 中允许的三个键的解析结果；`null` 表示未设置或非法，调用方取默认值。 */
data class TermuxConfig(
    val workingDirectory: String? = null,
    val scrollbackLines: Int? = null,
    val cursorStyle: CursorStyle? = null,
)

object TermuxProperties {
    private const val MAX_BYTES = 64 * 1024

    fun load(context: android.content.Context): TermuxConfig = parse(File(context.filesDir, "home"))

    /** 可直接以 home 目录测试的纯函数。 */
    fun parse(home: File): TermuxConfig {
        val candidates = listOf(
            File(home, ".termux/termux.properties"),
            File(home, ".config/termux/termux.properties"),
        )
        val file = candidates.firstOrNull { isReadableRegular(it) } ?: return TermuxConfig()
        if (file.length() > MAX_BYTES) {
            LogUtil.warn("TermuxProperties", "config too large: ${file.absolutePath}")
            return TermuxConfig()
        }
        val props = Properties()
        try {
            file.inputStream().use { stream ->
                InputStreamReader(stream, StandardCharsets.UTF_8).use { reader ->
                    props.load(reader)
                }
            }
        } catch (exception: Exception) {
            LogUtil.warn("TermuxProperties", "failed to read ${file.absolutePath}", exception)
            return TermuxConfig()
        }
        val workingDirectory = props.getProperty("default-working-directory")?.trim()?.takeIf { it.isNotEmpty() }?.let { path ->
            val dir = File(path)
            if (dir.exists() && dir.isDirectory) path else {
                LogUtil.warn("TermuxProperties", "invalid default-working-directory: $path")
                null
            }
        }
        val scrollbackLines = props.getProperty("terminal-transcript-rows")?.trim()?.toIntOrNull()?.let { value ->
            if (value in 100..50000) value else {
                LogUtil.warn("TermuxProperties", "invalid terminal-transcript-rows: $value")
                null
            }
        }
        val cursorStyle = props.getProperty("terminal-cursor-style")?.trim()?.lowercase()?.let { value ->
            when (value) {
                "block" -> CursorStyle.Block
                "bar", "beam" -> CursorStyle.Bar
                "underline" -> CursorStyle.Underline
                else -> {
                    LogUtil.warn("TermuxProperties", "invalid terminal-cursor-style: $value")
                    null
                }
            }
        }
        return TermuxConfig(workingDirectory, scrollbackLines, cursorStyle)
    }

    private fun isReadableRegular(file: File): Boolean {
        if (!file.exists() || !file.canRead()) return false
        val path = file.toPath()
        return try {
            java.nio.file.Files.isRegularFile(path, java.nio.file.LinkOption.NOFOLLOW_LINKS)
        } catch (exception: Exception) {
            false
        }
    }
}