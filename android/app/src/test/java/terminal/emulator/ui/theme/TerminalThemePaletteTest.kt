package terminal.emulator.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * BuiltInThemes.byName — the theme-name resolver with its fallback.
 */
class TerminalThemePaletteTest {

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
