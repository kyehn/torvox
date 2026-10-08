package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `renderWithNewOutput` 打包位的**真实解码**（[Bridge.decodeRenderResult]）。
 *
 * 断言一律用字面量位形而非 `Bridge` 的常量：两侧同步改名的常量会恒真，
 * 测不到解码路径。位形与 Rust `renderWithNewOutput` 的打包一一对应：
 * 计数 0..31、`new_output` 32、光标行 33..42、内容下沿 43..52、失效位 53、备用屏 54。
 */
class RenderResultPackingTest {

    private fun decode(packed: Long) = decodeRenderResult(packed)

    private fun cursorBits(row: Int) = row.toLong() shl 33

    private fun contentBits(row: Int) = row.toLong() shl 43

    @Test
    fun `count and new output come from the low 32 bits`() {
        val result = decode(7L or (1L shl 32))
        assertEquals(7, result.count)
        assertTrue(result.newOutput)
    }

    @Test
    fun `a negative count is preserved as the render failure value`() {
        // 原生出错时返回 -1（计数位全 1），Kotlin 侧据此判失败而非空闲帧。
        assertEquals(-1, decode(-1L).count)
    }

    @Test
    fun `idle frame with row sentinels decodes to unknown rows`() {
        // 原生在无帧缓存时写入的正是两个哨兵（0x3FF），故空闲帧的打包值带哨兵；
        // 解码须把它们映射为 -1 而非行号 1023。
        val result = decode(cursorBits(0x3FF) or contentBits(0x3FF))
        assertFalse(result.newOutput)
        assertEquals(Bridge.CURSOR_ROW_UNKNOWN, result.cursorRow)
        assertEquals(Bridge.LAST_CONTENT_ROW_NONE, result.lastContentRow)
        assertFalse(result.surfaceInvalidated)
        assertFalse(result.altScreenActive)
    }

    @Test
    fun `a row field never reads the neighbouring field's bits`() {
        // 光标行用 10 位掩码：内容下沿位（43..52）右移 33 后整体落在掩码之外，
        // 只能贡献 0，绝不会把内容下沿的 0x2A 读成光标行号。
        val result = decode(contentBits(0x2A))
        assertEquals(0, result.cursorRow)
        assertEquals(0x2A, result.lastContentRow)
        assertFalse(result.altScreenActive)
    }

    @Test
    fun `cursor and content rows decode from their own fields`() {
        val result = decode(cursorBits(7) or contentBits(5))
        assertEquals(7, result.cursorRow)
        assertEquals(5, result.lastContentRow)
    }

    @Test
    fun `row sentinels decode to the unknown constants`() {
        // 0x3FF 是「光标隐藏/视口外」与「视口全空」两个哨兵，解码须映射为 -1。
        val result = decode(cursorBits(0x3FF) or contentBits(0x3FF))
        assertEquals(Bridge.CURSOR_ROW_UNKNOWN, result.cursorRow)
        assertEquals(Bridge.LAST_CONTENT_ROW_NONE, result.lastContentRow)
    }

    @Test
    fun `alt screen bit 54 decodes without touching the other fields`() {
        // 备用屏置位时，输出标志/光标行/内容下沿/失效位必须仍是原值：
        // 位宽越界（例如内容下沿掩码写成 12 位）会让它被读成行号。
        val result = decode((1L shl 54) or cursorBits(9) or contentBits(11))
        assertTrue(result.altScreenActive)
        assertFalse(result.newOutput)
        assertEquals(9, result.cursorRow)
        assertEquals(11, result.lastContentRow)
        assertFalse(result.surfaceInvalidated)
    }

    @Test
    fun `surface invalidated bit 53 decodes without touching the other fields`() {
        val result = decode((1L shl 53) or cursorBits(3))
        assertTrue(result.surfaceInvalidated)
        assertFalse(result.altScreenActive)
        assertEquals(3, result.cursorRow)
    }

    @Test
    fun `alt screen and surface invalidated are independent`() {
        // 两个布尔标志可同时为真，宿主据此换窗口的同时置零位移。
        val result = decode((1L shl 54) or (1L shl 53))
        assertTrue(result.surfaceInvalidated)
        assertTrue(result.altScreenActive)
    }
}