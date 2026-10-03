package terminal.emulator.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * BuiltInThemes theme-name resolution. Unknown names must surface as errors, not
 * be silently swapped for a default: an unresolvable stored name is corrupt
 * settings data, which DESIGN:16 requires clearing rather than papering over.
 */
class TerminalThemePaletteTest {

    @Test
    fun `byNameOrNull resolves a known theme`() {
        assertEquals(
            BuiltInThemes.catppuccinMocha,
            BuiltInThemes.byNameOrNull(BuiltInThemes.catppuccinMocha.name),
        )
    }

    @Test
    fun `byNameOrNull returns null for unknown and empty names`() {
        assertNull(BuiltInThemes.byNameOrNull("no-such-theme"))
        assertNull(BuiltInThemes.byNameOrNull(""))
    }

    @Test
    fun `byName throws for an unknown name instead of silently substituting dracula plus`() {
        assertThrows(IllegalStateException::class.java) { BuiltInThemes.byName("no-such-theme") }
    }

    @Test
    fun `every built-in theme is reachable by its own name`() {
        for (theme in BuiltInThemes.all) {
            assertEquals(theme, BuiltInThemes.byNameOrNull(theme.name))
        }
    }
}
