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

data class TerminalTheme(
    val name: String,
    val background: Color,
    val foreground: Color,
    val cursor: Color,
    val selectionBackground: Color = Color(0xFF45475A),
    val ansi: List<Color>,
) {
    init {
        require(ansi.size == 16) { "Theme must have exactly 16 ANSI colors" }
    }
}

object BuiltInThemes {
    val draculaPlus =
        TerminalTheme(
            name = "Dracula Plus",
            background = Color(0xFF212121),
            foreground = Color(0xFFF8F8F2),
            cursor = Color(0xFFECEFF4),
            selectionBackground = Color(0xFF44475A),
            ansi =
            listOf(
                Color(0xFF21222C),
                Color(0xFFFF5555),
                Color(0xFF50FA7B),
                Color(0xFFFFCB6B),
                Color(0xFF82AAFF),
                Color(0xFFC792EA),
                Color(0xFF8BE9FD),
                Color(0xFFF8F9F2),
                Color(0xFF545454),
                Color(0xFFFF6E6E),
                Color(0xFF69FF94),
                Color(0xFFFFCB6B),
                Color(0xFFD6ACFF),
                Color(0xFFFF92DF),
                Color(0xFFA4FFFF),
                Color(0xFFF8F8F2),
            ),
        )

    val catppuccinMocha =
        TerminalTheme(
            name = "Catppuccin Mocha",
            background = Color(0xFF1E1E2E),
            foreground = Color(0xFFCDD6F4),
            cursor = Color(0xFFF5E0DC),
            selectionBackground = Color(0xFF45475A),
            ansi =
            listOf(
                Color(0xFF45475A),
                Color(0xFFF38BA8),
                Color(0xFFA6E3A1),
                Color(0xFFF9E2AF),
                Color(0xFF89B4FA),
                Color(0xFFF5C2E7),
                Color(0xFF94E2D5),
                Color(0xFFBAC2DE),
                Color(0xFF585B70),
                Color(0xFFF38BA8),
                Color(0xFFA6E3A1),
                Color(0xFFF9E2AF),
                Color(0xFF89B4FA),
                Color(0xFFF5C2E7),
                Color(0xFF94E2D5),
                Color(0xFFA6ADC8),
            ),
        )

    val catppuccinLatte =
        TerminalTheme(
            name = "Catppuccin Latte",
            background = Color(0xFFEFF1F5),
            foreground = Color(0xFF4C4F69),
            cursor = Color(0xFFDC8A78),
            selectionBackground = Color(0xFFCCD0DA),
            ansi =
            listOf(
                Color(0xFF5C5F77),
                Color(0xFFD20F39),
                Color(0xFF40A02B),
                Color(0xFFDF8E1D),
                Color(0xFF1E66F5),
                Color(0xFFEA76CB),
                Color(0xFF179299),
                Color(0xFFACB0BE),
                Color(0xFF6C6F85),
                Color(0xFFD20F39),
                Color(0xFF40A02B),
                Color(0xFFDF8E1D),
                Color(0xFF1E66F5),
                Color(0xFFEA76CB),
                Color(0xFF179299),
                Color(0xFFBCC0CC),
            ),
        )

    val tokyoNight =
        TerminalTheme(
            name = "Tokyo Night",
            background = Color(0xFF1A1B26),
            foreground = Color(0xFFA9B1D6),
            cursor = Color(0xFFA9B1D6),
            selectionBackground = Color(0xFF2F3B54),
            ansi =
            listOf(
                Color(0xFF32344A),
                Color(0xFFF7768E),
                Color(0xFF9ECE6A),
                Color(0xFFE0AF68),
                Color(0xFF7AA2F7),
                Color(0xFFAD8EE6),
                Color(0xFF449DAB),
                Color(0xFF787C99),
                Color(0xFF444B6A),
                Color(0xFFFF7A93),
                Color(0xFFB9F27C),
                Color(0xFFFF9E64),
                Color(0xFF7DA6FF),
                Color(0xFFBB9AF7),
                Color(0xFF0DB9D7),
                Color(0xFFACB0D0),
            ),
        )

    val gruvboxDark =
        TerminalTheme(
            name = "Gruvbox Dark",
            background = Color(0xFF282828),
            foreground = Color(0xFFEBDBB2),
            cursor = Color(0xFFEBDBB2),
            selectionBackground = Color(0xFF3C3836),
            ansi =
            listOf(
                Color(0xFF282828),
                Color(0xFFCC241D),
                Color(0xFF98971A),
                Color(0xFFD79921),
                Color(0xFF458588),
                Color(0xFFB16286),
                Color(0xFF689D6A),
                Color(0xFFA89984),
                Color(0xFF928374),
                Color(0xFFFB4934),
                Color(0xFFB8BB26),
                Color(0xFFFABD2F),
                Color(0xFF83A598),
                Color(0xFFD3869B),
                Color(0xFF8EC07C),
                Color(0xFFEBDBB2),
            ),
        )

    val gruvboxLight =
        TerminalTheme(
            name = "Gruvbox Light",
            background = Color(0xFFFBF1C7),
            foreground = Color(0xFF3C3836),
            cursor = Color(0xFF3C3836),
            selectionBackground = Color(0xFFEBDBB2),
            ansi =
            listOf(
                Color(0xFFFBF1C7),
                Color(0xFFCC241D),
                Color(0xFF98971A),
                Color(0xFFD79921),
                Color(0xFF458588),
                Color(0xFFB16286),
                Color(0xFF689D6A),
                Color(0xFF7C6F64),
                Color(0xFF928374),
                Color(0xFF9D0006),
                Color(0xFF79740E),
                Color(0xFFB57614),
                Color(0xFF076678),
                Color(0xFF8F3F71),
                Color(0xFF427B58),
                Color(0xFF3C3836),
            ),
        )

    val monokai =
        TerminalTheme(
            name = "Monokai",
            background = Color(0xFF272822),
            foreground = Color(0xFFF8F8F2),
            cursor = Color(0xFFF8F8F2),
            selectionBackground = Color(0xFF3E3D32),
            ansi =
            listOf(
                Color(0xFF272822),
                Color(0xFFF92672),
                Color(0xFFA6E22E),
                Color(0xFFF4BF75),
                Color(0xFF66D9EF),
                Color(0xFFAE81FF),
                Color(0xFFA1EFE4),
                Color(0xFFF8F8F2),
                Color(0xFF75715E),
                Color(0xFFF92672),
                Color(0xFFA6E22E),
                Color(0xFFF4BF75),
                Color(0xFF66D9EF),
                Color(0xFFAE81FF),
                Color(0xFFA1EFE4),
                Color(0xFFF9F8F5),
            ),
        )

    // 取自 alacritty-theme 的 tomorrow.toml；源主题无 selection 段，按前景/背景 3:7 混合取选区底色。
    val tomorrow =
        TerminalTheme(
            name = "Tomorrow",
            background = Color(0xFFFFFFFF),
            foreground = Color(0xFF4D4D4C),
            cursor = Color(0xFFD6D6D6),
            selectionBackground = Color(0xFFCACAC9),
            ansi =
            listOf(
                Color(0xFF1D1F21),
                Color(0xFFC82829),
                Color(0xFF718C00),
                Color(0xFFF5871F),
                Color(0xFF4271AE),
                Color(0xFF8959A8),
                Color(0xFF3E999F),
                Color(0xFFD6D6D6),
                Color(0xFF8E908C),
                Color(0xFFFF3334),
                Color(0xFF89AA00),
                Color(0xFFEAB700),
                Color(0xFF5795E6),
                Color(0xFFB777E0),
                Color(0xFF66BDC3),
                Color(0xFFEFEFEF),
            ),
        )

    // 取自 alacritty-theme 的 tomorrow_night.toml；源主题无 selection 段，按前景/背景 3:7 混合取选区底色。
    val tomorrowNight =
        TerminalTheme(
            name = "Tomorrow Night",
            background = Color(0xFF1D1F21),
            foreground = Color(0xFFC5C8C6),
            cursor = Color(0xFFFFFFFF),
            selectionBackground = Color(0xFF4F5252),
            ansi =
            listOf(
                Color(0xFF1D1F21),
                Color(0xFFCC6666),
                Color(0xFFB5BD68),
                Color(0xFFE6C547),
                Color(0xFF81A2BE),
                Color(0xFFB294BB),
                Color(0xFF70C0BA),
                Color(0xFF373B41),
                Color(0xFF666666),
                Color(0xFFFF3334),
                Color(0xFF9EC400),
                Color(0xFFF0C674),
                Color(0xFF81A2BE),
                Color(0xFFB77EE0),
                Color(0xFF54CED6),
                Color(0xFF282A2E),
            ),
        )

    // 取自 alacritty-theme 的 tokyo_night_light.toml。
    val tokyoNightLight =
        TerminalTheme(
            name = "Tokyo Night Light",
            background = Color(0xFFD6D8DF),
            foreground = Color(0xFF343B58),
            cursor = Color(0xFF707280),
            selectionBackground = Color(0xFFACB0BF),
            ansi =
            listOf(
                Color(0xFF343B58),
                Color(0xFFC24242),
                Color(0xFF41A6B5),
                Color(0xFF8F5E15),
                Color(0xFF2959AA),
                Color(0xFF7B43BA),
                Color(0xFF006C86),
                Color(0xFF707280),
                Color(0xFF343B58),
                Color(0xFFC24242),
                Color(0xFF41A6B5),
                Color(0xFF8F5E15),
                Color(0xFF2959AA),
                Color(0xFF7B43BA),
                Color(0xFF006C86),
                Color(0xFF707280),
            ),
        )
    val darkThemes: List<TerminalTheme> =
        listOf(
            draculaPlus,
            catppuccinMocha,
            monokai,
            gruvboxDark,
            tokyoNight,
            tomorrowNight,
        )

    val lightThemes: List<TerminalTheme> =
        listOf(
            catppuccinLatte,
            gruvboxLight,
            tomorrow,
            tokyoNightLight,
        )

    val all: List<TerminalTheme> = darkThemes + lightThemes

    /**
     * 按名取主题。未知名返回 null：设置里存着无法解析的主题名即设置数据错误
     * （DESIGN:16「设置数据错误 → 清除设置数据」、:24「不做未要求的 Fallback」），
     * 静默替换成 draculaPlus 会让用户选了别的主题却看不出哪一环失配。
     * 调用方负责清除出错的那个键。
     */
    fun byNameOrNull(name: String): TerminalTheme? = all.firstOrNull { it.name == name }

    fun byName(name: String): TerminalTheme = byNameOrNull(name) ?: error("unknown terminal theme: $name")
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
