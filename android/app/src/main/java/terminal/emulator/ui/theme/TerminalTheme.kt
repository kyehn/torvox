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
 */
data class TerminalTheme(
    val name: String,
    /** 深底主题，供「跟随系统」在日夜两组间切换；判定由原生给出。 */
    val dark: Boolean,
    val background: Color,
    val foreground: Color,
    val cursor: Color,
    val selectionBackground: Color,
    val ansi: List<Color>,
) {
    init {
        require(ansi.size == 16) { "Theme must have exactly 16 ANSI colors" }
    }
}

/**
 * 终端主题模式取值（落盘于 `SettingsRepository` 的 `themeMode`）。
 *
 * 唯一声明处：`when` 的分支、设置页的选项与默认值都引用它们，写错字面量不再会
 * 悄悄落进 `else` 的「跟随系统」分支。
 */
object ThemeMode {
    /** 固定使用单一主题（`themeName` 键）。 */
    const val FIXED = "fixed"

    /** 固定使用日间主题（`dayThemeName` 键）。 */
    const val DAY = "day"

    /** 固定使用夜间主题（`nightThemeName` 键）。 */
    const val NIGHT = "night"

    /** 跟随软件主题在日间与夜间主题间切换。 */
    const val FOLLOW_SYSTEM = "follow_system"
}

/**
 * 由主题模式设置解析出活动的终端主题名。
 * 纯函数，供 TerminalScreen 的背景、TerminalRuntime 的设置读取与主题查找共用，
 * 使模式切换逻辑只存在于一处。
 */
fun resolveTerminalThemeName(
    mode: String,
    fixedName: String,
    dayName: String,
    nightName: String,
    isDark: Boolean,
): String = when (mode) {
    ThemeMode.FIXED -> fixedName
    ThemeMode.DAY -> dayName
    ThemeMode.NIGHT -> nightName
    else -> if (isDark) nightName else dayName
}

/** 由应用主题模式设置解析应用的深色模式。 */
fun resolveAppDarkMode(appThemeMode: String, systemDark: Boolean): Boolean = when (appThemeMode) {
    ThemeMode.NIGHT -> true
    ThemeMode.DAY -> false
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
        appThemeMode == ThemeMode.FOLLOW_SYSTEM -> {
            if (isDarkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }

        forceDark -> darkColorScheme()

        else -> lightColorScheme()
    }
}
