package terminal.emulator.ui.theme

import androidx.compose.ui.graphics.Color
import kotlinx.collections.immutable.ImmutableList
import kotlinx.collections.immutable.toImmutableList
import terminal.emulator.bridge.NativeBridge

/**
 * 内置终端主题仓库：单源在 Rust 侧（构建期嵌入的 alacritty 主题），
 * 经原生直接量按标识取回，不在 Kotlin 侧另存名字与颜色副本，不经 JSON。
 */
object ThemeRepository {
    /** 原生主题打包字节数：深底标记 1 + 背景 3 + 前景 3 + 光标 3 + 选区 3 + 十六色 48。 */
    const val PACKED_THEME_BYTES = 61

    /** 十六色数量。 */
    private const val ANSI_COLOR_COUNT = 16

    /** 单色字节数。 */
    private const val COLOR_BYTES = 3

    /** 颜色在打包中的偏移（首字节是深底标记）。 */
    private const val COLOR_OFFSET = 1

    /** 背景在打包中的偏移。 */
    private const val BACKGROUND_OFFSET = COLOR_OFFSET

    /** 前景在打包中的偏移。 */
    private const val FOREGROUND_OFFSET = BACKGROUND_OFFSET + COLOR_BYTES

    /** 光标在打包中的偏移。 */
    private const val CURSOR_OFFSET = FOREGROUND_OFFSET + COLOR_BYTES

    /** 选区在打包中的偏移。 */
    private const val SELECTION_OFFSET = CURSOR_OFFSET + COLOR_BYTES

    /** 十六色在打包中的偏移。 */
    private const val ANSI_OFFSET = SELECTION_OFFSET + COLOR_BYTES

    /**
     * 由原生打包字节构造主题。纯函数，不碰 JNI，可直接单测。
     * 长度不符即抛异常，不截断、不补默认色。
     */
    fun themeFromPacked(themeName: String, packedTheme: ByteArray): TerminalTheme {
        require(packedTheme.size == PACKED_THEME_BYTES) { "Packed theme must be $PACKED_THEME_BYTES bytes" }

        fun colorAt(colorOffset: Int): Color {
            val redChannel = packedTheme[colorOffset].toInt() and 0xFF
            val greenChannel = packedTheme[colorOffset + 1].toInt() and 0xFF
            val blueChannel = packedTheme[colorOffset + 2].toInt() and 0xFF
            val packedArgb =
                0xFF000000L or
                    (redChannel.toLong() shl 16) or
                    (greenChannel.toLong() shl 8) or
                    blueChannel.toLong()
            return Color(packedArgb)
        }

        val ansiColors = List(ANSI_COLOR_COUNT) { ansiIndex ->
            colorAt(ANSI_OFFSET + ansiIndex * COLOR_BYTES)
        }
        return TerminalTheme(
            name = themeName,
            dark = packedTheme[0].toInt() != 0,
            background = colorAt(BACKGROUND_OFFSET),
            foreground = colorAt(FOREGROUND_OFFSET),
            cursor = colorAt(CURSOR_OFFSET),
            selectionBackground = colorAt(SELECTION_OFFSET),
            ansi = ansiColors,
        )
    }

    /** 全部主题，深底在前、日间在后（与原生顺序一致）。 */
    fun all(): List<TerminalTheme> {
        val names =
            NativeBridge.getThemeNames()
                ?: error("Theme names unavailable from native")
        return names.map { themeName -> byName(themeName) }
    }

    /**
     * 进程内缓存的 [all]。主题在构建期定进二进制，进程生命周期内不变，而 [all] 要
     * 经 JNI 取名并逐主题取色——调用方若用 `remember` 持有，列表项滚出屏幕再滚回
     * 就会重跑整轮（十一次同步 JNI）。
     */
    val allCached: ImmutableList<TerminalTheme> by lazy { all().toImmutableList() }

    /**
     * 按名取主题。未知返回 null（设置数据错误，由调用方清除该键）；
     * 原生已认识该标识却取不到颜色则抛异常，不伪装成未知。
     */
    fun byNameOrNull(themeName: String): TerminalTheme? {
        val packedTheme = NativeBridge.getThemeColors(themeName) ?: return null
        return themeFromPacked(themeName, packedTheme)
    }

    /** 按名取主题，未知抛异常，不静默替换默认主题。 */
    fun byName(themeName: String): TerminalTheme =
        byNameOrNull(themeName) ?: error("unknown terminal theme: $themeName")

    /** 默认夜间主题标识，对应默认终端主题。 */
    fun defaultDarkName(): String = NativeBridge.getDefaultDarkName() ?: error("Unknown default dark theme")

    /** 默认日间主题标识。 */
    fun defaultLightName(): String = NativeBridge.getDefaultLightName() ?: error("Unknown default light theme")
}
