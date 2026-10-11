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
 * 由主题模式设置解析出活动的终端主题名。
 * 纯函数，供 TerminalScreen 的背景与主题查找共用，使模式切换逻辑只存在于一处。
 */
fun resolveTerminalThemeName(
    mode: String,
    fixedName: String,
    dayName: String,
    nightName: String,
    isDark: Boolean,
): String = when (mode) {
    "fixed" -> fixedName
    "day" -> dayName
    "night" -> nightName
    else -> if (isDark) nightName else dayName
}

/** 由应用主题模式设置解析应用的深色模式。 */
fun resolveAppDarkMode(appThemeMode: String, systemDark: Boolean): Boolean = when (appThemeMode) {
    "night" -> true
    "day" -> false
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
        appThemeMode == "follow_system" -> {
            if (isDarkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }

        forceDark -> darkColorScheme()

        else -> lightColorScheme()
    }
}
