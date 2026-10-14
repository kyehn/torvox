package terminal.emulator.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 主题名解析的纯逻辑：模式切换只做字符串选择，不碰 JNI、不读硬编码表。
 * 未知名是否报错由调用方（ThemeRepository.byName / 设置清理）裁决，
 * 此处只锁定“同名输入得到同名输出”的透传语义。
 */
class TerminalThemePaletteTest {

    @Test
    fun `fixed mode passes through the stored name`() {
        assertEquals(
            "test_theme",
            resolveTerminalThemeName(
                mode = "fixed",
                fixedName = "test_theme",
                dayName = "test_day",
                nightName = "test_night",
                isDark = true,
            ),
        )
    }

    @Test
    fun `follow system picks night or day by darkness`() {
        assertEquals(
            "test_night",
            resolveTerminalThemeName(
                mode = "follow_system",
                fixedName = "test_theme",
                dayName = "test_day",
                nightName = "test_night",
                isDark = true,
            ),
        )
        assertEquals(
            "test_day",
            resolveTerminalThemeName(
                mode = "follow_system",
                fixedName = "test_theme",
                dayName = "test_day",
                nightName = "test_night",
                isDark = false,
            ),
        )
    }

    @Test
    fun `packed theme keeps its own name`() {
        val name = "test_theme".toByteArray(Charsets.UTF_8)
        val entry =
            byteArrayOf(
                (
                    ThemeRepository.FLAG_DARK or ThemeRepository.FLAG_DEFAULT_DARK or
                        ThemeRepository.FLAG_DEFAULT_LIGHT
                    ).toByte(),
                name.size.toByte(),
            ) +
                name +
                ByteArray(54) { 0x01 }
        val catalog = byteArrayOf(1, 0) + entry
        assertEquals("test_theme", ThemeRepository.parseCatalog(catalog).themes.single().name)
    }
}
