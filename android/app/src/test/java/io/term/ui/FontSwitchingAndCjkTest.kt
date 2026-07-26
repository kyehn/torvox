package io.term.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class FontSwitchingAndCjkTest {
    @Test
    fun fontListIncludesAtLeastOneMonospaceFont() {
        val fonts = fallbackSystemFonts()
        val hasMono =
            fonts.any {
                it.contains("Mono", ignoreCase = true) ||
                    it.contains("Droid", ignoreCase = true)
            }
        assertTrue("Font list should include a monospace font, got: ${fonts.take(10)}", hasMono)
    }

    @Test
    fun fontListIncludesCjkOrMaple() {
        val fonts = fallbackSystemFonts()
        val hasCjk =
            fonts.any {
                it.contains("CJK", ignoreCase = true) ||
                    it.contains("Maple", ignoreCase = true) ||
                    it.contains("SC", ignoreCase = true)
            }
        assertTrue("Font list should include a CJK-capable font, got: ${fonts.take(10)}", hasCjk)
    }

    @Test
    fun fontListHasNoDuplicates() {
        val fonts = fallbackSystemFonts()
        val lower = fonts.map { it.lowercase() }
        val unique = lower.toSet()
        assertTrue("Font list has duplicates: ${fonts.size} vs ${unique.size} unique", lower.size == unique.size)
    }

    @Test
    fun fontListHasMinimumSize() {
        val fonts = fallbackSystemFonts()
        assertTrue("Font list should have at least 7 fonts, got ${fonts.size}", fonts.size >= 7)
    }

    @Test
    fun fallbackSystemFontsAllNonEmpty() {
        val fonts = fallbackSystemFonts()
        assertTrue("Font list should not be empty", fonts.isNotEmpty())
        assertFalse("No font name should be blank", fonts.any { it.isBlank() })
    }

    @Test
    fun fallbackSystemFontsIncludesDroidSansMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Should include Droid Sans Mono",
            fonts.any { it.equals("Droid Sans Mono", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFontsIncludesNotoSansMono() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Should include Noto Sans Mono",
            fonts.any { it.contains("Noto Sans Mono", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFontsIncludesCjkFonts() {
        val fonts = fallbackSystemFonts()
        val cjkFonts =
            fonts.filter {
                it.contains("CJK", ignoreCase = true) ||
                    it.contains("Chinese", ignoreCase = true) ||
                    it.contains("Maple", ignoreCase = true) ||
                    it.contains("Noto Sans SC", ignoreCase = true)
            }
        assertTrue(
            "Should have at least 2 CJK-capable fonts, found: $cjkFonts",
            cjkFonts.size >= 2,
        )
    }
}
