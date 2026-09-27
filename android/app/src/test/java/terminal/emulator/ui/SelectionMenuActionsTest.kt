package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 选择菜单链接/文件项显示阈值（纯逻辑，不查可用性）。
 */
class SelectionMenuActionsTest {
    @Test
    fun `OSC8超链接去除首尾空白`() {
        assertEquals("https://osc8.example/", resolveOpenLinkUri("  https://osc8.example/  "))
    }

    @Test
    fun `无超链接或空白超链接返回null`() {
        assertNull(resolveOpenLinkUri(null))
        assertNull(resolveOpenLinkUri("   "))
    }

    @Test
    fun `超长选择不显示文件项`() {
        assertFalse(isFilePathCandidate("/" + "a".repeat(2048)))
    }

    @Test
    fun `绝对路径显示打开文件`() {
        assertTrue(isFilePathCandidate("/data/data/com.termux/files/home/.termux/font.ttf"))
    }

    @Test
    fun `相对路径不显示打开文件`() {
        assertFalse(isFilePathCandidate("home/file.txt"))
    }

    @Test
    fun `shell错误行不显示打开文件`() {
        assertFalse(isFilePathCandidate("/system/bin/sh: helloworldtest8: inaccessible or not found"))
    }

    @Test
    fun `含空格文本不显示打开文件`() {
        assertFalse(isFilePathCandidate("/data/data/com.termux/files/home/my file.txt"))
    }
}
