package io.torvox.ui.theme

import android.os.Build
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

data class TerminalTheme(
    val name: String,
    val background: Color,
    val foreground: Color,
    val cursor: Color,
    val selectionBg: Color = Color(0xFF45475A),
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
            selectionBg = Color(0xFF44475A),
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
            selectionBg = Color(0xFF45475A),
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
            selectionBg = Color(0xFFCCD0DA),
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

    val nord =
        TerminalTheme(
            name = "Nord",
            background = Color(0xFF2E3440),
            foreground = Color(0xFFD8DEE9),
            cursor = Color(0xFFD8DEE9),
            selectionBg = Color(0xFF434C5E),
            ansi =
            listOf(
                Color(0xFF3B4252),
                Color(0xFFBF616A),
                Color(0xFFA3BE8C),
                Color(0xFFEBCB8B),
                Color(0xFF81A1C1),
                Color(0xFFB48EAD),
                Color(0xFF88C0D0),
                Color(0xFFE5E9F0),
                Color(0xFF4C566A),
                Color(0xFFBF616A),
                Color(0xFFA3BE8C),
                Color(0xFFEBCB8B),
                Color(0xFF81A1C1),
                Color(0xFFB48EAD),
                Color(0xFF8FBCBB),
                Color(0xFFECEFF4),
            ),
        )

    val tokyoNight =
        TerminalTheme(
            name = "Tokyo Night",
            background = Color(0xFF1A1B26),
            foreground = Color(0xFFA9B1D6),
            cursor = Color(0xFFA9B1D6),
            selectionBg = Color(0xFF2F3B54),
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

    val rosePine =
        TerminalTheme(
            name = "Rose Pine",
            background = Color(0xFF191724),
            foreground = Color(0xFFE0DEF4),
            cursor = Color(0xFF524F67),
            selectionBg = Color(0xFF2A273F),
            ansi =
            listOf(
                Color(0xFF26233A),
                Color(0xFFEB6F92),
                Color(0xFF31748F),
                Color(0xFFF6C177),
                Color(0xFF9CCFD8),
                Color(0xFFC4A7E7),
                Color(0xFFEBBCBA),
                Color(0xFFE0DEF4),
                Color(0xFF6E6A86),
                Color(0xFFEB6F92),
                Color(0xFF31748F),
                Color(0xFFF6C177),
                Color(0xFF9CCFD8),
                Color(0xFFC4A7E7),
                Color(0xFFEBBCBA),
                Color(0xFFE0DEF4),
            ),
        )

    val gruvboxDark =
        TerminalTheme(
            name = "Gruvbox Dark",
            background = Color(0xFF282828),
            foreground = Color(0xFFEBDBB2),
            cursor = Color(0xFFEBDBB2),
            selectionBg = Color(0xFF3C3836),
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
            selectionBg = Color(0xFFEBDBB2),
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

    val everforestDark =
        TerminalTheme(
            name = "Everforest Dark",
            background = Color(0xFF2D353B),
            foreground = Color(0xFFD3C6AA),
            cursor = Color(0xFFD3C6AA),
            selectionBg = Color(0xFF3D484D),
            ansi =
            listOf(
                Color(0xFF475258),
                Color(0xFFE67E80),
                Color(0xFFA7C080),
                Color(0xFFDBBC7F),
                Color(0xFF7FBBB3),
                Color(0xFFD699B6),
                Color(0xFF83C092),
                Color(0xFFD3C6AA),
                Color(0xFF475258),
                Color(0xFFE67E80),
                Color(0xFFA7C080),
                Color(0xFFDBBC7F),
                Color(0xFF7FBBB3),
                Color(0xFFD699B6),
                Color(0xFF83C092),
                Color(0xFFD3C6AA),
            ),
        )

    val oneDark =
        TerminalTheme(
            name = "One Dark",
            background = Color(0xFF282C34),
            foreground = Color(0xFFABB2BF),
            cursor = Color(0xFFABB2BF),
            selectionBg = Color(0xFF3E4451),
            ansi =
            listOf(
                Color(0xFF1E2127),
                Color(0xFFE06C75),
                Color(0xFF98C379),
                Color(0xFFD19A66),
                Color(0xFF61AFEF),
                Color(0xFFC678DD),
                Color(0xFF56B6C2),
                Color(0xFFABB2BF),
                Color(0xFF5C6370),
                Color(0xFFE06C75),
                Color(0xFF98C379),
                Color(0xFFD19A66),
                Color(0xFF61AFEF),
                Color(0xFFC678DD),
                Color(0xFF56B6C2),
                Color(0xFFFFFFFF),
            ),
        )

    val oneLight =
        TerminalTheme(
            name = "One Light",
            background = Color(0xFFF8F8F8),
            foreground = Color(0xFF2A2B33),
            cursor = Color(0xFF2A2B33),
            selectionBg = Color(0xFFE0E0E0),
            ansi =
            listOf(
                Color(0xFF000000),
                Color(0xFFDE3D35),
                Color(0xFF3E953A),
                Color(0xFFD2B67B),
                Color(0xFF2F5AF3),
                Color(0xFFA00095),
                Color(0xFF3E953A),
                Color(0xFFBBBBBB),
                Color(0xFF000000),
                Color(0xFFDE3D35),
                Color(0xFF3E953A),
                Color(0xFFD2B67B),
                Color(0xFF2F5AF3),
                Color(0xFFA00095),
                Color(0xFF3E953A),
                Color(0xFFFFFFFF),
            ),
        )

    val monokai =
        TerminalTheme(
            name = "Monokai",
            background = Color(0xFF272822),
            foreground = Color(0xFFF8F8F2),
            cursor = Color(0xFFF8F8F2),
            selectionBg = Color(0xFF3E3D32),
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

    val ayuDark =
        TerminalTheme(
            name = "Ayu Dark",
            background = Color(0xFF0A0E14),
            foreground = Color(0xFFB3B1AD),
            cursor = Color(0xFFB3B1AD),
            selectionBg = Color(0xFF1A1F29),
            ansi =
            listOf(
                Color(0xFF01060E),
                Color(0xFFEA6C73),
                Color(0xFF91B362),
                Color(0xFFF9AF4F),
                Color(0xFF53BDFA),
                Color(0xFFFAE994),
                Color(0xFF90E1C6),
                Color(0xFFC7C7C7),
                Color(0xFF686868),
                Color(0xFFF07178),
                Color(0xFFC2D94C),
                Color(0xFFFFB454),
                Color(0xFF59C2FF),
                Color(0xFFFFEE99),
                Color(0xFF95E6CB),
                Color(0xFFFFFFFF),
            ),
        )

    val ayuLight =
        TerminalTheme(
            name = "Ayu Light",
            background = Color(0xFFFCFCFC),
            foreground = Color(0xFF5C6166),
            cursor = Color(0xFF5C6166),
            selectionBg = Color(0xFFE8E8E8),
            ansi =
            listOf(
                Color(0xFF010101),
                Color(0xFFE7666A),
                Color(0xFF80AB24),
                Color(0xFFEBA54D),
                Color(0xFF4196DF),
                Color(0xFF9870C3),
                Color(0xFF51B891),
                Color(0xFFC1C1C1),
                Color(0xFF343434),
                Color(0xFFEE9295),
                Color(0xFF9FD32F),
                Color(0xFFF0BC7B),
                Color(0xFF6DAEE6),
                Color(0xFFB294D2),
                Color(0xFF75C7A8),
                Color(0xFFDBDBDB),
            ),
        )

    val kanagawaWave =
        TerminalTheme(
            name = "Kanagawa Wave",
            background = Color(0xFF1F1F28),
            foreground = Color(0xFFDCD7BA),
            cursor = Color(0xFFDCD7BA),
            selectionBg = Color(0xFF2D2D3F),
            ansi =
            listOf(
                Color(0xFF090618),
                Color(0xFFC34043),
                Color(0xFF76946A),
                Color(0xFFC0A36E),
                Color(0xFF7E9CD8),
                Color(0xFF957FB8),
                Color(0xFF6A9589),
                Color(0xFFC8C093),
                Color(0xFF727169),
                Color(0xFFE82424),
                Color(0xFF98BB6C),
                Color(0xFFE6C384),
                Color(0xFF7FB4CA),
                Color(0xFF938AA9),
                Color(0xFF7AA89F),
                Color(0xFFDCD7BA),
            ),
        )

    val nightOwl =
        TerminalTheme(
            name = "Night Owl",
            background = Color(0xFF011627),
            foreground = Color(0xFFD6DEEB),
            cursor = Color(0xFFD6DEEB),
            selectionBg = Color(0xFF0B2D4A),
            ansi =
            listOf(
                Color(0xFF011627),
                Color(0xFFEF5350),
                Color(0xFF22DA6E),
                Color(0xFFC5E478),
                Color(0xFF82AAFF),
                Color(0xFFC792EA),
                Color(0xFF21C7A8),
                Color(0xFFFFFFFF),
                Color(0xFF575656),
                Color(0xFFEF5350),
                Color(0xFF22DA6E),
                Color(0xFFFFEB95),
                Color(0xFF82AAFF),
                Color(0xFFC792EA),
                Color(0xFF7FDBCA),
                Color(0xFFFFFFFF),
            ),
        )

    val darkThemes: List<TerminalTheme> =
        listOf(
            draculaPlus,
            catppuccinMocha,
            nord,
            tokyoNight,
            rosePine,
            gruvboxDark,
            everforestDark,
            oneDark,
            monokai,
            ayuDark,
            kanagawaWave,
            nightOwl,
        )

    val lightThemes: List<TerminalTheme> =
        listOf(
            catppuccinLatte,
            gruvboxLight,
            oneLight,
            ayuLight,
        )

    val all: List<TerminalTheme> = darkThemes + lightThemes

    fun byName(name: String): TerminalTheme = all.firstOrNull { it.name == name } ?: catppuccinMocha
}

enum class ThemeMode(
    val label: String,
) {
    DAY("Day"),
    NIGHT("Night"),
    FOLLOW_SYSTEM("Follow System"),
}

@Composable
fun dynamicTerminalTheme(isDark: Boolean): TerminalTheme? {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return null
    val context = LocalContext.current
    val scheme = if (isDark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    return TerminalTheme(
        name = if (isDark) "Material You Dark" else "Material You Light",
        background = scheme.background,
        foreground = scheme.onBackground,
        cursor = scheme.primary,
        selectionBg = scheme.surfaceVariant,
        ansi =
        listOf(
            scheme.errorContainer,
            scheme.error,
            scheme.primary,
            scheme.tertiary,
            scheme.secondary,
            scheme.tertiaryContainer,
            scheme.primaryContainer,
            scheme.onBackground,
            scheme.outlineVariant,
            scheme.error,
            scheme.primary,
            scheme.tertiary,
            scheme.secondary,
            scheme.tertiaryContainer,
            scheme.primaryContainer,
            scheme.onSurfaceVariant,
        ),
    )
}
