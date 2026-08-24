package terminal.emulator.ui.theme

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * TerminalTheme.generate256Palette + BuiltInThemes.byName — the xterm
 * 256-color palette (16 ANSI + 6×6×6 cube + 24-step grayscale) and the
 * theme-name resolver with its fallback.
 */
class TerminalThemePaletteTest {

    @Test
    fun `palette has exactly 256 entries`() {
        assertEquals(256, TerminalTheme.generate256Palette().size)
    }

    @Test
    fun `first sixteen entries are the ansi colors`() {
        val palette = TerminalTheme.generate256Palette()
        assertEquals(Color(0xFF000000), palette[0])
        assertEquals(Color(0xFFC0C0C0), palette[7])
        assertEquals(Color(0xFFFF0000), palette[9])
        assertEquals(Color(0xFFFFFFFF), palette[15])
    }

    @Test
    fun `cube entries use the xterm channel values`() {
        val palette = TerminalTheme.generate256Palette()
        // index = 16 + r*36 + g*6 + b, channels 0/95/135/175/215/255.
        assertEquals(Color(0xFF000000), palette[16]) // cube(0,0,0)
        assertEquals(Color(0xFF0000FF), palette[21]) // cube(0,0,5): b=255
        assertEquals(Color(0xFF5F0000), palette[52]) // cube(1,0,0): r=95
        assertEquals(Color(0xFFFF0000), palette[196]) // cube(5,0,0): r=255
        assertEquals(Color(0xFFFFFFFF), palette[231]) // cube(5,5,5)
    }

    @Test
    fun `grayscale ramp runs 8 to 238 step 10`() {
        val palette = TerminalTheme.generate256Palette()
        assertEquals(Color(0xFF080808), palette[232])
        assertEquals(Color(0xFFEEEEEE), palette[255])
        // Midpoint: 232 + 11 = 243 → 8 + 110 = 118.
        assertEquals(Color(0xFF767676), palette[243])
    }

    @Test
    fun `every entry carries full alpha`() {
        val palette = TerminalTheme.generate256Palette()
        assertTrue(palette.all { it.alpha == 1f })
    }

    @Test
    fun `byName resolves a known theme`() {
        assertEquals(BuiltInThemes.catppuccinMocha, BuiltInThemes.byName(BuiltInThemes.catppuccinMocha.name))
    }

    @Test
    fun `byName falls back to catppuccin mocha for unknown names`() {
        assertEquals(BuiltInThemes.catppuccinMocha, BuiltInThemes.byName("no-such-theme"))
        assertEquals(BuiltInThemes.catppuccinMocha, BuiltInThemes.byName(""))
    }
}
