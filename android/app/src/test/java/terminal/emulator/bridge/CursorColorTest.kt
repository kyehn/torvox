package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test

class CursorColorTest {
    private fun assertRgb(
        expected: FloatArray,
        actual: FloatArray,
    ) {
        assertEquals(expected.size, actual.size)
        for (i in expected.indices) {
            assertEquals("channel $i", expected[i], actual[i], 0.0001f)
        }
    }

    @Test
    fun `white maps to unit channels`() {
        assertRgb(floatArrayOf(1f, 1f, 1f), argbToRgbFloats(0xFFFFFFFF.toInt()))
    }

    @Test
    fun `black maps to zero channels`() {
        assertRgb(floatArrayOf(0f, 0f, 0f), argbToRgbFloats(0xFF000000.toInt()))
    }

    @Test
    fun `pure red ignores green and blue`() {
        assertRgb(floatArrayOf(1f, 0f, 0f), argbToRgbFloats(0xFFFF0000.toInt()))
    }

    @Test
    fun `alpha byte is dropped`() {
        // 0x00123456 — alpha 0x00 must not affect the RGB output.
        assertRgb(floatArrayOf(18f / 255f, 52f / 255f, 86f / 255f), argbToRgbFloats(0x00123456))
    }

    @Test
    fun `mid gray round trips through the byte range`() {
        // 0x80 = 128/255 ≈ 0.502, not exactly 0.5.
        assertRgb(floatArrayOf(128f / 255f, 128f / 255f, 128f / 255f), argbToRgbFloats(0xFF808080.toInt()))
    }
}
