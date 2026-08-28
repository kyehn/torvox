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
}
