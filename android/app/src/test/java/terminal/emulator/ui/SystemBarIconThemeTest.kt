package terminal.emulator.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.ui.theme.TerminalTheme

/**
 * 系统栏图标明暗必须跟随**终端主题背景像素**，而不是系统深浅色或软件主题开关
 * （窗口边到边，系统栏只决定图标明暗；日间 + 浅色终端主题下图标恒为浅色即不可见）。
 *
 * 用假数据构造主题，不读原生表：清单构成由构建期 alacritty-theme 裁决，
 * 点名真实主题会把当前清单焊死在测试里。
 */
class SystemBarIconThemeTest {

    private fun fakeTheme(themeName: String, background: Color): TerminalTheme = TerminalTheme.fromPackedColors(
        themeName,
        byteArrayOf(background.redByte(), background.greenByte(), background.blueByte()) +
            ByteArray(TerminalTheme.PACKED_COLOR_BYTES - 3) { 0x01 },
    )

    private fun Color.redByte(): Byte = (this.red * 255f + 0.5f).toInt().toByte()

    private fun Color.greenByte(): Byte = (this.green * 255f + 0.5f).toInt().toByte()

    private fun Color.blueByte(): Byte = (this.blue * 255f + 0.5f).toInt().toByte()

    @Test
    fun `light background asks for dark icons`() {
        assertTrue(usesLightSystemBarIcons(Color(0xFFFFFFFF)))
        assertTrue(usesLightSystemBarIcons(Color(0xFFF8F8F2)))
    }

    @Test
    fun `dark background asks for light icons`() {
        assertFalse(usesLightSystemBarIcons(Color(0xFF000000)))
        assertFalse(usesLightSystemBarIcons(Color(0xFF101020)))
    }

    /**
     * 主题背景的图标明暗必须与亮度自洽：浅背景一律深色图标，深背景一律浅色图标。
     */
    @Test
    fun `fake themes get icons matching their own background luminance`() {
        val themes =
            listOf(
                fakeTheme("test_light", Color(0xFFFFFFFF)),
                fakeTheme("test_dark", Color(0xFF101020)),
            )
        val lightThemes = themes.filter { usesLightSystemBarIcons(it.background) }.map { it.name }
        assertTrue(
            "浅色主题不应为空，否则本用例无判别力",
            lightThemes.isNotEmpty(),
        )
        assertTrue(
            "深色主题不应为空，否则本用例无判别力",
            lightThemes.size < themes.size,
        )
        for (theme in themes) {
            val light = usesLightSystemBarIcons(theme.background)
            val luminance = theme.background.luminance()
            assertTrue(
                "主题 ${theme.name} 判据与亮度自洽：light=$light luminance=$luminance",
                (light && luminance > 0.5f) || (!light && luminance <= 0.5f),
            )
        }
    }

    /** 深色背景下图标必须是浅色。 */
    @Test
    fun `dark background uses light icons`() {
        assertFalse(usesLightSystemBarIcons(Color(0xFF101020)))
    }
}
