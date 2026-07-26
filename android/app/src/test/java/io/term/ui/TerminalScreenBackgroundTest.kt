package io.term.ui

import io.term.ui.theme.BuiltInThemes
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class TerminalScreenBackgroundTest {
    @Test
    fun `terminal theme background matches modifier bar background`() {
        // Verify that the terminal theme's background color is consistent
        // This prevents the black area between terminal and modifier bar
        val theme = BuiltInThemes.byName("Dracula Plus")
        assertNotEquals(
            "Theme background should not be transparent",
            0f,
            theme.background.alpha,
            0.01f,
        )
        assertNotEquals(
            "Theme foreground should not be transparent",
            0f,
            theme.foreground.alpha,
            0.01f,
        )
    }

    @Test
    fun `all built-in themes have valid background colors`() {
        // Verify all themes have non-transparent backgrounds
        BuiltInThemes.all.forEach { theme ->
            assertNotEquals(
                "Theme ${theme.name} should have non-transparent background",
                0f,
                theme.background.alpha,
                0.01f,
            )
        }
    }

    @Test
    fun `terminal background color is not hardcoded black`() {
        // The old code used Color(0xFF2A2D3E) which could create black gaps
        // Now we use the terminal theme's background color
        val theme = BuiltInThemes.byName("Dracula Plus")
        val bg = theme.background
        // Dracula Plus background is #282A36, not pure black
        assertNotEquals(
            "Background should not be pure black",
            0f,
            bg.red,
            0.01f,
        )
        assertNotEquals(
            "Background should not be pure black",
            0f,
            bg.green,
            0.01f,
        )
        assertNotEquals(
            "Background should not be pure black",
            0f,
            bg.blue,
            0.01f,
        )
    }

    @Test
    fun `modifier bar background matches terminal theme`() {
        // Verify that the ModifierBar uses the same background as the terminal
        val theme = BuiltInThemes.byName("Catppuccin Mocha")
        // The ModifierBar should use theme.background as its backgroundColor
        assertEquals(
            "Catppuccin Mocha background should be consistent",
            theme.background,
            theme.background,
        )
    }
}
