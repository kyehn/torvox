package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * `getCellSizePacked` 打包值的**真实解码**（[unpackCellSize]）。
 *
 * 断言一律用字面量位形而非 `Bridge` 的常量：两侧同步改名的常量会恒真，测不到解码
 * 路径。位形与 Rust `getCellSizePacked` 的打包一一对应：低 32 位宽度、高 32 位高度，
 * 各为该 `f32` 的 IEEE-754 位。整格字号（如 8.5×17.25）在两个半区都取回原值，
 * 且宽度的符号位不得渗进高度半区。
 */
class CellSizePackingTest {
    private fun pack(width: Float, height: Float): Long {
        val widthBits = width.toRawBits().toLong() and 0xFFFFFFFFL
        return ((height.toRawBits().toLong() and 0xFFFFFFFFL) shl 32) or widthBits
    }

    @Test
    fun `width and height come from the low and high halves`() {
        val (width, height) = unpackCellSize(pack(8.5f, 17.25f))
        assertEquals(8.5f, width, 0f)
        assertEquals(17.25f, height, 0f)
    }

    @Test
    fun `width sign bit does not leak into the height half`() {
        val (width, height) = unpackCellSize(pack(Float.NEGATIVE_INFINITY, 12f))
        assertEquals(Float.NEGATIVE_INFINITY, width, 0f)
        assertEquals(12f, height, 0f)
    }

    @Test
    fun `zero means the renderer is not ready yet`() {
        val (width, height) = unpackCellSize(0L)
        assertEquals(0f, width, 0f)
        assertEquals(0f, height, 0f)
    }
}
