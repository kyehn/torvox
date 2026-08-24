package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SystemFontsTest {

    @Test
    fun cleanFontName_preserves_already_clean_name() {
        assertEquals("Roboto Mono", cleanFontName("Roboto Mono"))
        assertEquals("Noto Sans SC", cleanFontName("Noto Sans SC"))
    }

    @Test
    fun cleanFontName_replaces_underscores_and_hyphens_with_spaces() {
        assertEquals("jetbrains mono", cleanFontName("jetbrains_mono"))
        assertEquals("noto sans mono", cleanFontName("noto-sans-mono"))
        assertEquals("a b c", cleanFontName("a_b-c"))
    }

    @Test
    fun cleanFontName_trims_surrounding_whitespace() {
        assertEquals("spaced", cleanFontName("  spaced  "))
    }

    @Test
    fun cleanFontName_returns_null_when_result_is_empty() {
        assertNull(cleanFontName(""))
        assertNull(cleanFontName("_"))
        assertNull(cleanFontName("---"))
        assertNull(cleanFontName("   "))
    }
}
