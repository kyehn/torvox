package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

class DerivedDefaultFontSizeTest {
    @Test
    fun phone_width_yields_10sp_default() {
        // 360dp / 60 columns / 0.6 aspect = 10sp — matches the historical fixed default.
        assertEquals(10f, derivedDefaultFontSp(360), 0.01f)
    }

    @Test
    fun tablet_width_yields_larger_font() {
        // 600dp / 60 / 0.6 = 16.7sp — a tablet shows the same column count, not tiny text.
        assertEquals(16.67f, derivedDefaultFontSp(600), 0.01f)
    }

    @Test
    fun narrow_screen_is_clamped_to_minimum() {
        // 200dp / 60 / 0.6 = 5.6sp → clamped to 8sp (readable floor).
        assertEquals(8f, derivedDefaultFontSp(200), 0.01f)
    }

    @Test
    fun huge_screen_is_clamped_to_maximum() {
        // 1200dp / 60 / 0.6 = 33sp → clamped to 18sp.
        assertEquals(18f, derivedDefaultFontSp(1200), 0.01f)
    }
}
