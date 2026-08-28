package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round-234 tests for [SettingsRepository.defaultFontSizeFor] and the
 * [SettingsRepository.MIN_FONT_SP] floor (spec default-typography "首启默认字号下限": a fresh install never
 * launches below the floor — termux default_font_size parity). Round-237 raised the floor to 14sp
 * and the cap to 24sp (user-reported "default too small").
 */
class DefaultFontSizeForTest {

    private fun expected(widthDp: Float): Float = (widthDp / 52f / 0.6f).coerceIn(SettingsRepository.MIN_FONT_SP, 24f)

    @Test
    fun smallPhoneNeverBelowFloor() {
        // 360dp phone: raw formula gives ~11.5sp — the floor must lift it to 12.
        val size = SettingsRepository.defaultFontSizeFor(360f)
        assertTrue("360dp must be >= MIN_FONT_SP, got $size", size >= SettingsRepository.MIN_FONT_SP)
        assertEquals(SettingsRepository.MIN_FONT_SP, size, 0.001f)
    }

    @Test
    fun emulatorWidthUsesFormula() {
        // 411dp (1080px @420dpi): raw = 13.17sp, above the floor.
        assertEquals(expected(411f), SettingsRepository.defaultFontSizeFor(411f), 0.001f)
    }

    @Test
    fun tabletClampsToMax() {
        assertEquals(24f, SettingsRepository.defaultFontSizeFor(900f), 0.001f)
    }

    @Test
    fun degenerateWidthStillAtFloor() {
        assertEquals(SettingsRepository.MIN_FONT_SP, SettingsRepository.defaultFontSizeFor(0f), 0.001f)
        assertEquals(
            SettingsRepository.MIN_FONT_SP,
            SettingsRepository.defaultFontSizeFor(-100f),
            0.001f,
        )
    }
}
