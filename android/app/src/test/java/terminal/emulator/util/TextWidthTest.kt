package terminal.emulator.util

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TextWidthTest {
    @Test
    fun `ascii is narrow`() {
        assertFalse(isWideChar('a'))
        assertFalse(isWideChar('0'))
        assertFalse(isWideChar(' '))
        assertEquals(1, charCellWidth('a'))
    }

    @Test
    fun `cjk codepoints are wide`() {
        assertTrue(isWideChar('中'))
        assertTrue(isWideChar('文'))
        assertTrue(isWideChar('你'))
        assertEquals(2, charCellWidth('中'))
    }

    @Test
    fun `hangul jamo and syllables are wide`() {
        assertTrue(isWideCodePoint(0x1100)) // Hangul Jamo onset
        assertTrue(isWideChar('한')) // Hangul syllable
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
        assertEquals(1, charCellWidth('\u0301'))
    }

    @Test
    fun `charIndexToCellColumn sums widths`() {
        // "a中b": columns a=0, 中=1-2, b=3
        assertEquals(0, charIndexToCellColumn("a中b", 0))
        assertEquals(1, charIndexToCellColumn("a中b", 1))
        assertEquals(3, charIndexToCellColumn("a中b", 2))
        assertEquals(4, charIndexToCellColumn("a中b", 3))
    }

    @Test
    fun `charIndexToCellColumn clamps out of range index`() {
        assertEquals(4, charIndexToCellColumn("a中b", 99))
        assertEquals(0, charIndexToCellColumn("", 0))
        assertEquals(0, charIndexToCellColumn("", 5))
    }

    @Test
    fun `charIndexToCellColumn handles empty line`() {
        assertEquals(0, charIndexToCellColumn("", 0))
    }

    @Test
    fun `emoji surrogate pair width via char cell width`() {
        // "😀" is two UTF-16 chars; each surrogate is a narrow code unit
        assertEquals(1, charCellWidth('\uD83D'))
        assertEquals(1, charCellWidth('\uDE00'))
        // The logical character is wide when classified by code point
        assertTrue(isWideCodePoint(0x1F600))
    }

    @Test
    fun `charIndexToCellColumn with mixed emoji and cjk`() {
        // "😀中" as UTF-16: surrogate(0) + surrogate(1) + 中(2); each surrogate
        // is one cell, 中 is two cells → total 4 columns.
        assertEquals(0, charIndexToCellColumn("😀中", 0))
        assertEquals(1, charIndexToCellColumn("😀中", 1))
        assertEquals(2, charIndexToCellColumn("😀中", 2))
        assertEquals(4, charIndexToCellColumn("😀中", 3))
    }
}
