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
 * 计数 0..31、`new_output` 32、内容下沿 43..52、失效位 53、备用屏 54。
 *
 * 33..42 是光标行的历史位形：输入法位移改按内容下沿裁剪后该字段已无消费方，
 * 随之从原生打包与本解码一并删除。**必须留空**——一旦有人挪动内容下沿的偏移，
 * 下面按字面量写的断言会立刻判红。
 */
class RenderResultPackingTest {

    private fun decode(packed: Long) = decodeRenderResult(packed)

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
    fun `idle frame with the content sentinel decodes to unknown`() {
        // 原生在无帧缓存时写入的正是哨兵（0x3FF），故空闲帧的打包值带哨兵；
        // 解码须把它映射为 -1 而非行号 1023。
        val result = decode(contentBits(0x3FF))
        assertFalse(result.newOutput)
        assertEquals(Bridge.LAST_CONTENT_ROW_NONE, result.lastContentRow)
        assertFalse(result.surfaceInvalidated)
        assertFalse(result.altScreenActive)
    }

    @Test
    fun `output bit never reads the content row bits`() {
        // 输出位是 32，只屏蔽第 32 位：内容下沿（43..52）右移 32 后落在更高位，
        // 掩码取不到；若解码写成 shr 31 之类的错位，空闲帧会被误判为有新输出。
        assertFalse(decode(contentBits(0x2A)).newOutput)
    }

    @Test
    fun `content row decodes from its own field`() {
        assertEquals(5, decode(contentBits(5)).lastContentRow)
    }

    @Test
    fun `content row never reads the flag bits above it`() {
        // 内容下沿用 10 位掩码：失效位（53）、备用屏（54）右移 43 后分别落在
        // 第 10、11 位，掩码取不到，绝不会把标志读成行号。
        val result = decode((1L shl 53) or (1L shl 54) or contentBits(0x2A))
        assertTrue(result.surfaceInvalidated)
        assertTrue(result.altScreenActive)
        assertEquals(0x2A, result.lastContentRow)
    }

    @Test
    fun `alt screen bit 54 decodes without touching the other fields`() {
        // 备用屏置位时，输出标志/内容下沿/失效位必须仍是原值：
        // 位宽越界（例如内容下沿掩码写成 12 位）会让它被读成行号。
        val result = decode((1L shl 54) or contentBits(11))
        assertTrue(result.altScreenActive)
        assertFalse(result.newOutput)
        assertEquals(11, result.lastContentRow)
        assertFalse(result.surfaceInvalidated)
    }

    @Test
    fun `surface invalidated bit 53 decodes without touching the other fields`() {
        val result = decode((1L shl 53) or contentBits(3))
        assertTrue(result.surfaceInvalidated)
        assertFalse(result.altScreenActive)
        // 位 53 右移 43 后落在掩码外，只会贡献 0；带一个非零内容下沿才看得出
        // 它没被读进行号字段（raw=0 本身与合法的第 0 行无法区分）。
        assertEquals(3, result.lastContentRow)
    }

    @Test
    fun `alt screen and surface invalidated are independent`() {
        // 两个布尔标志可同时为真，宿主据此换窗口的同时置零位移。
        val result = decode((1L shl 54) or (1L shl 53))
        assertTrue(result.surfaceInvalidated)
        assertTrue(result.altScreenActive)
    }
}
