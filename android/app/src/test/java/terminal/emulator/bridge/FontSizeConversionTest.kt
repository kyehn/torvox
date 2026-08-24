package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test

class FontSizeConversionTest {
    @Test
    fun `sp to px scales with density`() {
        assertEquals(14f, fontSpToPx(14f, density = 1f))
        assertEquals(38.5f, fontSpToPx(14f, density = 2.75f))
        assertEquals(0f, fontSpToPx(0f, density = 3f))
    }

    @Test
    fun `px to sp converts back at the same density`() {
        assertEquals(14f, fontPxToSp(38.5f, density = 2.75f))
        assertEquals(12f, fontPxToSp(36f, density = 3f))
    }

    @Test
    fun `round trip is lossless at practical densities`() {
        for (density in listOf(1f, 1.5f, 2f, 2.75f, 3f, 3.5f)) {
            for (sizeSp in listOf(8f, 12f, 14f, 16f, 20f, 32f)) {
                val roundTripped = fontPxToSp(fontSpToPx(sizeSp, density), density)
                assertEquals("sp→px→sp round trip at density=$density", sizeSp, roundTripped, 0.001f)
            }
        }
    }

    @Test
    fun `non-integer sp value survives the boundary`() {
        val px = fontSpToPx(13.5f, density = 2f)
        assertEquals(27f, px)
        assertEquals(13.5f, fontPxToSp(px, density = 2f), 0.001f)
    }
}
