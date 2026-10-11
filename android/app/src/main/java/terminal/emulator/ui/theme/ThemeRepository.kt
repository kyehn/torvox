package terminal.emulator.ui.theme

import androidx.compose.ui.graphics.Color
import terminal.emulator.bridge.NativeBridge

/**
 * 内置终端主题仓库：单源在 Rust 侧（构建期嵌入的 alacritty 主题），
 * 经原生直接量逐项取回，不在 Kotlin 侧另存名字与颜色副本，不经 JSON。
 */
object ThemeRepository {
    /** 原生主题颜色字节数：背景 3 + 前景 3 + 光标 3 + 选区 3 + 十六色 48。 */
    const val PACKED_THEME_BYTES = 60

    /** 十六色数量。 */
    private const val ANSI_COLOR_COUNT = 16

    /** 单色字节数。 */
    private const val COLOR_BYTES = 3

    /** 背景在打包中的偏移。 */
    private const val BACKGROUND_OFFSET = 0

    /** 前景在打包中的偏移。 */
    private const val FOREGROUND_OFFSET = 3

    /** 光标在打包中的偏移。 */
    private const val CURSOR_OFFSET = 6

    /** 选区在打包中的偏移。 */
    private const val SELECTION_OFFSET = 9

    /** 十六色在打包中的偏移。 */
    private const val ANSI_OFFSET = 12

    /**
     * 由原生 60 字节构造主题。纯函数，不碰 JNI，可直接单测。
     * 长度不符即抛异常，不截断、不补默认色。
     */
    fun themeFromPacked(themeName: String, isDark: Boolean, packedColors: ByteArray): TerminalTheme {
        require(packedColors.size == PACKED_THEME_BYTES) { "Theme colors must be 60 bytes" }

        fun colorAt(colorOffset: Int): Color {
            val redChannel = packedColors[colorOffset].toInt() and 0xFF
            val greenChannel = packedColors[colorOffset + 1].toInt() and 0xFF
            val blueChannel = packedColors[colorOffset + 2].toInt() and 0xFF
            val packedArgb = 0xFF000000L or (redChannel.toLong() shl 16) or
                (greenChannel.toLong() shl 8) or blueChannel.toLong()
            return Color(packedArgb)
        }

        val ansiColors = List(ANSI_COLOR_COUNT) { ansiIndex ->
            colorAt(ANSI_OFFSET + ansiIndex * COLOR_BYTES)
        }
        return TerminalTheme(
            name = themeName,
            background = colorAt(BACKGROUND_OFFSET),
            foreground = colorAt(FOREGROUND_OFFSET),
            cursor = colorAt(CURSOR_OFFSET),
            selectionBackground = colorAt(SELECTION_OFFSET),
            ansi = ansiColors,
        )
    }

    /** 全部主题，深底在前、日间在后（与原生排序一致）。 */
    fun all(): List<TerminalTheme> {
        val themeCount = NativeBridge.getThemeCount()
        return List(themeCount) { themeIndex ->
            val themeName = NativeBridge.getThemeName(themeIndex)
                ?: error("Unknown theme at index $themeIndex")
            val isDark = NativeBridge.isThemeDark(themeIndex)
            val packedColors = NativeBridge.getThemeColors(themeIndex)
                ?: error("Unknown theme colors at index $themeIndex")
            themeFromPacked(themeName, isDark, packedColors)
        }
    }

    /**
     * 按名取主题。未知返回 null（设置数据错误，由调用方清除该键），
     * 有效序号下取数失败则抛异常，不伪装成未知。
     */
    fun byNameOrNull(themeName: String): TerminalTheme? {
        val themeIndex = NativeBridge.getThemeIndexByName(themeName)
        if (themeIndex < 0) return null
        val resolvedName = NativeBridge.getThemeName(themeIndex)
            ?: error("Unknown theme at index $themeIndex")
        val isDark = NativeBridge.isThemeDark(themeIndex)
        val packedColors = NativeBridge.getThemeColors(themeIndex)
            ?: error("Unknown theme colors at index $themeIndex")
        return themeFromPacked(resolvedName, isDark, packedColors)
    }

    /** 按名取主题，未知抛异常，不静默替换默认主题。 */
    fun byName(themeName: String): TerminalTheme =
        byNameOrNull(themeName) ?: error("unknown terminal theme: $themeName")

    /** 默认夜间主题标识，对应默认终端主题。 */
    fun defaultDarkName(): String = NativeBridge.getDefaultDarkName() ?: error("Unknown default dark theme")

    /** 默认日间主题标识。 */
    fun defaultLightName(): String = NativeBridge.getDefaultLightName() ?: error("Unknown default light theme")
}
