package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * 选择菜单链接项显示阈值（纯逻辑，不查可用性）。
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
}
