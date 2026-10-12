package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * 字体信息打包字节的真实解码（`FontInfo.fromPacked`）。
 *
 * 布局与原生 `pack_font_info` 一一对应：`u32` 标志（bit0 等宽、其余为 CJK 状态
 * 判别序号）+ 三个 `f32` + `u16` 名称数 + NUL 分隔的 UTF-8 名称。
 */
class FontInfoPackedTest {
    companion object {
        /** 固定部分字节数：标志 4 + 三个 `f32` 12 + 名称数 2。 */
        private const val HEADER_BYTES = 18
    }

    private fun pack(
        names: List<String>,
        monospaced: Boolean = false,
        cjkState: CjkState = CjkState.NONE,
        cellWidthPx: Float = 7.0f,
        cellHeightPx: Float = 14.0f,
        fontSizeSp: Float = 14.0f,
    ): ByteArray {
        val buffer = ByteBuffer.allocate(HEADER_BYTES)
        buffer.order(ByteOrder.LITTLE_ENDIAN)
        var flags = cjkState.ordinal shl 1
        if (monospaced) flags = flags or 1
        buffer.putInt(flags)
        buffer.putFloat(cellWidthPx)
        buffer.putFloat(cellHeightPx)
        buffer.putFloat(fontSizeSp)
        buffer.putShort(names.size.toShort())
        val nameBytes = ByteArrayOutputStream()
        names.forEach { name ->
            nameBytes.write(name.toByteArray(Charsets.UTF_8))
            nameBytes.write(0)
        }
        return buffer.array() + nameBytes.toByteArray()
    }

    @Test
    fun `packed bytes decode into flags metrics and names`() {
        val info =
            FontInfo.fromPacked(
                pack(
                    names = listOf("Fira Code", "Noto Sans CJK SC", "Noto Sans CJK JP"),
                    monospaced = true,
                    cjkState = CjkState.FALLBACK,
                ),
            )
        assertEquals("Fira Code", info.activeName)
        assertTrue(info.activeMonospaced)
        assertEquals(CjkState.FALLBACK, info.cjkState)
        assertEquals(listOf("Noto Sans CJK SC", "Noto Sans CJK JP"), info.cjkFamilies)
        assertEquals(7.0f, info.cellWidthPx, 0.0f)
        assertEquals(14.0f, info.cellHeightPx, 0.0f)
        assertEquals(14.0f, info.fontSizeSp, 0.0f)
        assertEquals("Noto Sans CJK SC, Noto Sans CJK JP", info.cjkFallbackText())
        assertTrue(info.hasRealCjkFallback)
    }

    @Test
    fun `non-ascii family names survive the utf8 round trip`() {
        val info = FontInfo.fromPacked(pack(names = listOf("等距更纱黑体", "思源黑体"), cjkState = CjkState.FALLBACK))
        assertEquals("等距更纱黑体", info.activeName)
        assertEquals(listOf("思源黑体"), info.cjkFamilies)
    }

    @Test
    fun `none state has no fallback`() {
        val info = FontInfo.fromPacked(pack(names = listOf("Liberation Mono"), monospaced = true))
        assertFalse(info.hasRealCjkFallback)
        assertNull(info.cjkFallbackText())
        assertEquals(CjkState.NONE, info.cjkState)
    }

    @Test
    fun `missing active font decodes as an empty name`() {
        val info = FontInfo.fromPacked(pack(names = listOf("")))
        assertEquals("", info.activeName)
        assertFalse(info.activeMonospaced)
    }

    @Test
    fun `truncated payload throws instead of decoding garbage`() {
        assertThrows(IllegalArgumentException::class.java) { FontInfo.fromPacked(ByteArray(10)) }
        // 名称数声明多于实际携带的名字时同样拒绝：截断的载荷会少报回退族。
        val truncated = pack(names = listOf("A", "B")).copyOf(HEADER_BYTES + 2)
        assertThrows(IllegalArgumentException::class.java) { FontInfo.fromPacked(truncated) }
    }
}
