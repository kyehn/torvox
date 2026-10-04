package terminal.emulator.util

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TextWidthTest {
    @Test
    fun `hangul jamo and syllables are wide`() {
        assertTrue(isWideCodePoint(0x1100)) // Hangul Jamo onset
        assertTrue(isWideCodePoint(0xAC00))
        assertTrue(isWideCodePoint(0xD7A3))
    }

    @Test
    fun `fullwidth forms are wide`() {
        assertTrue(isWideCodePoint(0xFF01)) // ！ fullwidth exclamation
        assertTrue(isWideCodePoint(0xFFE5)) // ￥ inside Fullwidth Signs (FFE0..FFE6)
        assertTrue(isWideCodePoint(0xFFE6)) // ￥ fullwidth yen sign
    }

    @Test
    fun `astral emoji are wide`() {
        assertTrue(isWideCodePoint(0x1F600)) // 😀
        assertTrue(isWideCodePoint(0x1F1E6)) // regional indicator A
        assertTrue(isWideCodePoint(0x1F680))
        assertTrue(isWideCodePoint(0x1F900))
        assertTrue(isWideCodePoint(0x20000)) // CJK ext B
        assertTrue(isWideCodePoint(0x3FFFD)) // CJK ext G
    }

    @Test
    fun `borders of wide ranges are exclusive`() {
        assertFalse(isWideCodePoint(0x10FF)) // just below Hangul Jamo
        assertFalse(isWideCodePoint(0x1160)) // just above Hangul Jamo
        assertFalse(isWideCodePoint(0x4DFF)) // just below CJK Unified
        assertFalse(isWideCodePoint(0x4E00 - 1)) // 0x9FFF is inside CJK Unified
        assertFalse(isWideCodePoint(0xA4CF + 1)) // just above Yi Syllables
        assertFalse(isWideCodePoint(0x1F1E5)) // below regional indicator
        assertFalse(isWideCodePoint(0x1FAFF + 1)) // above chess symbols
        assertFalse(isWideCodePoint(0x2FFFE)) // just above CJK ext B..F
    }

    @Test
    fun `combining and control chars are narrow`() {
        assertFalse(isWideCodePoint(0x0301)) // combining acute accent
        assertFalse(isWideCodePoint(0x0007)) // bell
    }
}
