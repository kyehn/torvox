package terminal.emulator.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.ui.usesLightSystemBarIcons

/**
 * 系统栏图标明暗必须跟随**终端主题背景像素**，而不是系统深浅色或软件主题开关
 * （窗口边到边，系统栏只决定图标明暗；日间 + 浅色终端主题下图标恒为浅色即不可见）。
 */
class SystemBarIconThemeTest {

    @Test
    fun `light background asks for dark icons`() {
        assertTrue(usesLightSystemBarIcons(Color(0xFFFFFFFF)))
        assertTrue(usesLightSystemBarIcons(Color(0xFFF8F8F2)))
    }

    @Test
    fun `dark background asks for light icons`() {
        assertFalse(usesLightSystemBarIcons(Color(0xFF000000)))
        assertFalse(usesLightSystemBarIcons(Color(0xFF1E1E2E)))
    }

    /**
     * 内置主题的图标明暗必须与背景亮度自洽：浅背景一律深色图标，深背景一律浅色图标。
     * 不点名具体主题：主题清单的构成由 D15 裁决，点名会把当前清单焊死在测试里。
     */
    @Test
    fun `every built-in theme gets icons matching its own background luminance`() {
        val lightThemes =
            BuiltInThemes.all.filter { usesLightSystemBarIcons(it.background) }.map { it.name }
        assertTrue(
            "内置浅色主题不应为空，否则本用例无判别力：${BuiltInThemes.all.map { it.name }}",
            lightThemes.isNotEmpty(),
        )
        assertTrue(
            "内置深色主题不应为空，否则本用例无判别力：${BuiltInThemes.all.map { it.name }}",
            lightThemes.size < BuiltInThemes.all.size,
        )
        for (theme in BuiltInThemes.all) {
            val light = usesLightSystemBarIcons(theme.background)
            val luminance = theme.background.luminance()
            assertTrue(
                "主题 ${theme.name} 判据与亮度自洽：light=$light luminance=$luminance",
                (light && luminance > 0.5f) || (!light && luminance <= 0.5f),
            )
        }
    }

    /** 默认主题是深色：首启与「重置为默认」路径下图标必须是浅色。 */
    @Test
    fun `default theme background is dark`() {
        assertFalse(usesLightSystemBarIcons(BuiltInThemes.draculaPlus.background))
    }
}
