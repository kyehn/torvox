package io.torvox.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class FontFallbackTest {

    @Test
    fun fallbackSystemFonts_returnsAtLeast7Names() {
        val fonts = fallbackSystemFonts()
        assertTrue("Expected >=7 fallback fonts, got ${fonts.size}", fonts.size >= 7)
    }

    @Test
    fun fallbackSystemFonts_hasNoDuplicates() {
        val fonts = fallbackSystemFonts()
        val lower = fonts.map { it.lowercase() }
        assertFalse(
            "Duplicate fonts found: ${fonts.groupBy { it.lowercase() }.filter { it.value.size > 1 }.keys}",
            lower.size != lower.toSet().size,
        )
    }

    @Test
    fun fallbackSystemFonts_includesJetBrainsMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected JetBrainsMono Nerd Font in fallback",
            fonts.any { it.equals("JetBrainsMono Nerd Font", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFonts_includesDroidSansMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected Droid Sans Mono in fallback",
            fonts.any { it.equals("Droid Sans Mono", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFonts_includesNotoSansMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected Noto Sans Mono in fallback",
            fonts.any { it.equals("Noto Sans Mono", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFonts_includesRobotoMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected Roboto Mono in fallback",
            fonts.any { it.equals("Roboto Mono", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFonts_includesSourceCodePro() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected Source Code Pro in fallback",
            fonts.any { it.equals("Source Code Pro", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFonts_includesFiraCode() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected Fira Code in fallback",
            fonts.any { it.equals("Fira Code", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFonts_includesUbuntuMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Expected Ubuntu Mono in fallback",
            fonts.any { it.equals("Ubuntu Mono", ignoreCase = true) },
        )
    }
}
