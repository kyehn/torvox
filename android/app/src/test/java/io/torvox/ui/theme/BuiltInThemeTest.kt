package io.torvox.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertSame
import org.junit.Test

class BuiltInThemeTest {
    @Test
    fun allThemesHaveCorrectAnsiCount() {
        for (theme in BuiltInThemes.all) {
            assertEquals(
                "${theme.name} must have 16 ANSI colors",
                16,
                theme.ansi.size,
            )
        }
    }

    @Test
    fun allThemesHaveNonEmptyName() {
        for (theme in BuiltInThemes.all) {
            assert(
                theme.name.isNotEmpty(),
            ) { "Theme name must not be empty" }
        }
    }

    @Test
    fun byNameReturnsExactMatch() {
        val theme = BuiltInThemes.byName("Dracula Plus")
        assertEquals("Dracula Plus", theme.name)
    }

    @Test
    fun byNameFallsBackToCatppuccinMochaOnUnknown() {
        val theme = BuiltInThemes.byName("Nonexistent Theme")
        assertSame(BuiltInThemes.catppuccinMocha, theme)
    }

    @Test
    fun byNameFallsBackToCatppuccinMochaOnEmpty() {
        val theme = BuiltInThemes.byName("")
        assertSame(BuiltInThemes.catppuccinMocha, theme)
    }

    @Test
    fun darkAndLightThemesAreDisjoint() {
        val darkNames = BuiltInThemes.darkThemes.map { it.name }.toSet()
        val lightNames = BuiltInThemes.lightThemes.map { it.name }.toSet()
        val intersection = darkNames.intersect(lightNames)
        assert(intersection.isEmpty()) {
            "Themes should not appear in both dark and light: $intersection"
        }
    }

    @Test
    fun allListIsUnionOfDarkAndLight() {
        assertEquals(
            BuiltInThemes.darkThemes.size + BuiltInThemes.lightThemes.size,
            BuiltInThemes.all.size,
        )
    }

    @Test
    fun defaultThemeNameIsInAllThemes() {
        val draculaPlus = BuiltInThemes.byName("Dracula Plus")
        assertNotNull(draculaPlus)
        assertEquals("Dracula Plus", draculaPlus.name)
    }

    @Test
    fun lightThemesHaveFourEntries() {
        assertEquals(4, BuiltInThemes.lightThemes.size)
    }

    @Test
    fun catppuccinMochaIsDarkTheme() {
        assert(BuiltInThemes.darkThemes.any { it.name == "Catppuccin Mocha" }) {
            "Catppuccin Mocha should be in darkThemes"
        }
    }
}
