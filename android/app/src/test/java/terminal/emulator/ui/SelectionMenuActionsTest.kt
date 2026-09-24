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
    fun `链接形态文本优先于OSC8超链接`() {
        assertEquals(
            "https://example.com/a",
            resolveOpenLinkUri("https://example.com/a", "https://other.example/"),
        )
    }

    @Test
    fun `非URL文本回退到选区起点的OSC8超链接`() {
        assertEquals(
            "https://osc8.example/",
            resolveOpenLinkUri("readme", "https://osc8.example/"),
        )
    }

    @Test
    fun `文本与超链接皆无返回null`() {
        assertNull(resolveOpenLinkUri("readme", null))
        assertNull(resolveOpenLinkUri("readme", "   "))
    }

    @Test
    fun `带引号的链接文本剥离包裹后仍优先`() {
        assertEquals(
            "https://example.com/a",
            resolveOpenLinkUri("\"https://example.com/a\"", null),
        )
    }

    @Test
    fun `http链接显示打开链接`() {
        assertTrue(isLinkTextCandidate("https://example.com/a?q=1"))
    }

    @Test
    fun `普通文本不显示打开链接`() {
        assertFalse(isLinkTextCandidate("hello world"))
    }

    @Test
    fun `超长选择不显示链接文件项`() {
        val long = "https://example.com/" + "a".repeat(2048)
        assertFalse(isLinkTextCandidate(long))
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
