package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `renderWithNewOutput` 的打包位解码：备用屏位（bit 54）必须独立于既有的输出位、
 * 光标行、内容下沿与 surface 失效位。
 *
 * 位形与 Rust `renderWithNewOutput` 的打包一一对应。断言一律作用于**掩码后的原始位**：
 * 哨兵位（0x3FF）映射为 -1 属解码层职责，直接比对 -1 会把语义与位形混为一谈。
 */
class RenderResultPackingTest {

    @Test
    fun `alt screen bit is bit 54 and disjoint from every other field`() {
        val altScreen = Bridge.ALT_SCREEN_ACTIVE_BIT
        assertEquals(54, java.lang.Long.numberOfTrailingZeros(altScreen))
        assertTrue("备用屏位与失效位重叠", altScreen and Bridge.SURFACE_INVALIDATED_BIT == 0L)
        assertTrue(
            "备用屏位落进光标行的 10 位掩码",
            altScreen and Bridge.CURSOR_ROW_HIDDEN_BITS.toLong() == 0L,
        )
        assertTrue(
            "备用屏位落进内容下沿的 10 位掩码",
            altScreen and Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong() == 0L,
        )
        // 输出位（bit 32）：污染它会击穿空闲闭锁。
        assertTrue("备用屏位落进输出位", altScreen shr 32 and 0x1L == 0L)
    }

    @Test
    fun `alt screen bit alone leaves the other fields zero`() {
        // 只置 bit 54：备用屏读出为 1，而输出标志、光标行、内容下沿、失效位
        // 的掩码结果必须全为 0——位宽越界（如内容下沿掩码写成 12 位）会读出行号。
        val packed = Bridge.ALT_SCREEN_ACTIVE_BIT
        assertEquals("备用屏位必须被读出", 1L, (packed shr 54) and 0x1L)
        assertEquals(0L, (packed shr 32) and 0x1L)
        assertEquals(0L, (packed shr 33) and Bridge.CURSOR_ROW_HIDDEN_BITS.toLong())
        assertEquals(0L, (packed shr 43) and Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong())
        assertEquals(0L, packed and Bridge.SURFACE_INVALIDATED_BIT)
    }

    @Test
    fun `sentinel rows survive alongside the alt screen bit`() {
        // 真实帧里光标行与内容下沿常取哨兵（0x3FF，空闲/隐藏时）。哨兵与备用屏
        // 同时出现时各自仍读回哨兵：否则备用屏会让空闲会话误报行号。
        val sentinel = (Bridge.CURSOR_ROW_HIDDEN_BITS.toLong() shl 33) or
            (Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong() shl 43) or
            Bridge.ALT_SCREEN_ACTIVE_BIT
        assertEquals(
            Bridge.CURSOR_ROW_HIDDEN_BITS.toLong(),
            (sentinel shr 33) and Bridge.CURSOR_ROW_HIDDEN_BITS.toLong(),
        )
        assertEquals(
            Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong(),
            (sentinel shr 43) and Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong(),
        )
        assertEquals(1L, (sentinel shr 54) and 0x1L)
    }

    @Test
    fun `primary screen leaves the alt screen bit clear`() {
        // 主屏（不置 bit 54）解码为 false，输入法位移公式原行为不变。
        val packed = (3L shl 33) or (5L shl 43)
        assertEquals(0L, packed and Bridge.ALT_SCREEN_ACTIVE_BIT)
        assertEquals(3L, (packed shr 33) and Bridge.CURSOR_ROW_HIDDEN_BITS.toLong())
        assertEquals(5L, (packed shr 43) and Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong())
    }
}
