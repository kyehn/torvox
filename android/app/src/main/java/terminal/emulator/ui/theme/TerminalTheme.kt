package terminal.emulator.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/**
 * 终端主题：名字与颜色一律经 [ThemeRepository] 从原生单源取回，
 * Kotlin 侧不另存任何名字面量与色值副本。
 *
 * 颜色由目录条目里那 54 字节（背景、前景各 3，十六色 48）在构造时解出，
 * 条目本身不保留——终端页的光标与选区由渲染器按同一份调色板绘制，
 * 设置页只用这里解出的背景、前景与十六色。
 */
data class TerminalTheme(val name: String, val background: Color, val foreground: Color, val ansi: List<Color>) {
    init {
        require(ansi.size == ANSI_COLOR_COUNT) { "Theme must have exactly $ANSI_COLOR_COUNT ANSI colors" }
    }

    companion object {
        private const val ANSI_COLOR_COUNT = 16
        private const val COLOR_BYTES = 3

        /** 目录条目里颜色的字节数：背景、前景与十六色。 */
        const val PACKED_COLOR_BYTES = (2 + ANSI_COLOR_COUNT) * COLOR_BYTES

        /**
         * 由目录条目里那段颜色字节构造主题。
         *
         * 长度不符即抛错：短数组会越界读出无意义颜色，长数组则是布局已变——
         * 两种都要当场出声，MUST NOT 截断或补默认色。
         */
        fun fromPackedColors(name: String, packedColors: ByteArray): TerminalTheme {
            require(packedColors.size == PACKED_COLOR_BYTES) {
                "Theme colours must be $PACKED_COLOR_BYTES bytes, got ${packedColors.size}"
            }
            return TerminalTheme(
                name = name,
                background = packedColors.colorAt(0),
                foreground = packedColors.colorAt(1),
                ansi = List(ANSI_COLOR_COUNT) { index -> packedColors.colorAt(2 + index) },
            )
        }

        private fun ByteArray.colorAt(index: Int): Color {
            val base = index * COLOR_BYTES
            return Color(
                0xFF000000L or
                    (this[base].toLong() and 0xFF shl 16) or
                    (this[base + 1].toLong() and 0xFF shl 8) or
                    (this[base + 2].toLong() and 0xFF),
            )
        }
    }
}

/**
 * 软件（应用外壳）主题模式取值，落盘于 `SettingsRepository` 的 `appThemeMode`。
 *
 * 唯一声明处：设置页选项、默认值与深色解析都引用它们，写错字面量不再会悄悄
 * 落进 `else` 的「跟随系统」分支。
 */
object AppThemeMode {
    /** 固定日间。 */
    const val DAY = "day"

    /** 固定夜间。 */
    const val NIGHT = "night"

    /** 跟随系统日夜。 */
    const val FOLLOW_SYSTEM = "follow_system"
}

/**
 * 终端主题模式取值，落盘于 `SettingsRepository` 的 `themeMode`。
 *
 * DESIGN 只给终端一个「日夜切换」开关：关＝固定用 `themeName`，
 * 开＝分别用 `dayThemeName`/`nightThemeName` 并跟随软件主题切换。
 */
object TerminalThemeMode {
    /** 开关关闭：固定使用单一主题（`themeName` 键）。 */
    const val FIXED = "fixed"

    /** 开关开启：按软件主题在日间/夜间主题间切换。 */
    const val DAY_NIGHT = "day_night"

    /**
     * 落盘值归一。
     *
     * 旧版把终端主题存成四态（`fixed`/`day`/`night`/`follow_system`），其中后三者
     * 在 UI 上都等价于「开关开启」。读取时统一折成两态，避免历史数据落进
     * 「不认识就走 else」的隐式分支。
     */
    fun normalize(stored: String): String = if (stored == FIXED) FIXED else DAY_NIGHT
}

/**
 * 由终端主题模式解析出活动主题名。
 * 纯函数，供 TerminalScreen 的背景、TerminalRuntime 的设置读取与主题查找共用，
 * 使模式切换逻辑只存在于一处。
 */
fun resolveTerminalThemeName(
    mode: String,
    fixedName: String,
    dayName: String,
    nightName: String,
    isDark: Boolean,
): String = if (TerminalThemeMode.normalize(mode) == TerminalThemeMode.DAY_NIGHT) {
    if (isDark) nightName else dayName
} else {
    fixedName
}

/** 由应用主题模式设置解析应用的深色模式。 */
fun resolveAppDarkMode(appThemeMode: String, systemDark: Boolean): Boolean = when (appThemeMode) {
    AppThemeMode.NIGHT -> true
    AppThemeMode.DAY -> false
    else -> systemDark
}

/**
 * 由应用主题模式设置解析 Material 3 配色方案。
 * 动态取色仅在 Android 12+ 且跟随系统时生效。
 */
@Composable
@ReadOnlyComposable
fun resolveMaterialColorScheme(appThemeMode: String, forceDark: Boolean, isDarkTheme: Boolean): ColorScheme {
    val context = LocalContext.current
    return when {
        appThemeMode == AppThemeMode.FOLLOW_SYSTEM -> {
            if (isDarkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }

        forceDark -> darkColorScheme()

        else -> lightColorScheme()
    }
}
