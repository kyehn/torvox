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
 * 判别序号）+ 三个 `f32` + `u16` 名称数 + `u16` CJK 名称数 + NUL 分隔的 UTF-8 名称。
 */
class FontInfoPackedTest {
    companion object {
        /** 固定部分字节数：标志 4 + 三个 `f32` 12 + 名称数 2 + CJK 名称数 2。 */
        private const val HEADER_BYTES = 20
    }

    private fun pack(
        names: List<String>,
        cjkCount: Int = 0,
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
        buffer.putShort(cjkCount.toShort())
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
                    cjkCount = 2,
                    monospaced = true,
                    cjkState = CjkState.FALLBACK,
                ),
            )
        assertEquals("Fira Code", info.activeName)
        assertTrue(info.activeMonospaced)
        assertEquals(CjkState.FALLBACK, info.cjkState)
        assertEquals(listOf("Noto Sans CJK SC", "Noto Sans CJK JP"), info.cjkFamilies)
        assertTrue(info.otherFamilies.isEmpty())
        assertEquals(7.0f, info.cellWidthPx, 0.0f)
        assertEquals(14.0f, info.cellHeightPx, 0.0f)
        assertEquals(14.0f, info.fontSizeSp, 0.0f)
        assertEquals("Noto Sans CJK SC, Noto Sans CJK JP", info.cjkFallbackText())
        assertTrue(info.hasRealCjkFallback)
    }

    /**
     * 「其他被使用字体」是名称表尾段，CJK 段紧随主字体之后——分隔全靠 CJK 名称数。
     * 若两侧都以「名称总数减一」切分，本用例即判红。
     */
    @Test
    fun `cjk count splits the trailing other families from the cjk block`() {
        val info =
            FontInfo.fromPacked(
                pack(
                    names = listOf("Droid Sans Mono", "Noto Sans CJK SC", "Noto Color Emoji", "DejaVu Sans Mono"),
                    cjkCount = 1,
                    cjkState = CjkState.FALLBACK,
                ),
            )
        assertEquals(listOf("Noto Sans CJK SC"), info.cjkFamilies)
        assertEquals(listOf("Noto Color Emoji", "DejaVu Sans Mono"), info.otherFamilies)
    }

    @Test
    fun `non-ascii family names survive the utf8 round trip`() {
        val info =
            FontInfo.fromPacked(
                pack(names = listOf("等距更纱黑体", "思源黑体", "霞鹜文楷"), cjkCount = 1, cjkState = CjkState.FALLBACK),
            )
        assertEquals("等距更纱黑体", info.activeName)
        assertEquals(listOf("思源黑体"), info.cjkFamilies)
        assertEquals(listOf("霞鹜文楷"), info.otherFamilies)
    }

    @Test
    fun `none state has no fallback`() {
        val info = FontInfo.fromPacked(pack(names = listOf("Liberation Mono"), monospaced = true))
        assertFalse(info.hasRealCjkFallback)
        assertNull(info.cjkFallbackText())
        assertEquals(CjkState.NONE, info.cjkState)
        assertTrue(info.otherFamilies.isEmpty())
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
        // CJK 名称数越过主字体占位即内部不一致：继续切分会崩在 subList 上，
        // 不如与截断同样以契约错误报出。
        val impossible = pack(names = listOf("A", "B"), cjkCount = 2)
        assertThrows(IllegalArgumentException::class.java) { FontInfo.fromPacked(impossible) }
    }

    /**
     * 未知 CJK 状态序号即契约不一致：原生与 Kotlin 的状态枚举对不上时必须报出，
     * 不能让下标越界抛一句无来由的异常。
     */
    @Test
    fun `unknown cjk state ordinal is rejected`() {
        val payload = pack(names = listOf("Liberation Mono"))
        val flags = (CjkState.entries.size shl 1)
        payload[0] = (flags and 0xFF).toByte()
        payload[1] = ((flags shr 8) and 0xFF).toByte()
        val error = assertThrows(IllegalArgumentException::class.java) { FontInfo.fromPacked(payload) }
        assertTrue("错误信息须指明越界的状态序号", error.message.orEmpty().contains("CJK state"))
    }
}
