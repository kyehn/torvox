package io.term

import org.junit.Assert.assertEquals
import org.junit.Test

class FontUtilsTest {
    @Test
    fun resolveEffectiveFontFamily_returnsAsIs() {
        assertEquals("", resolveEffectiveFontFamily(""))
        assertEquals("Roboto Mono", resolveEffectiveFontFamily("Roboto Mono"))
        assertEquals("My Custom Font", resolveEffectiveFontFamily("My Custom Font"))
        assertEquals("monospace", resolveEffectiveFontFamily("monospace"))
    }

    @Test
    fun resolveEffectiveFontFamily_resolvesGenericKeywords() {
        // K8: generic CSS keywords normalize to their canonical Android family.
        assertEquals("monospace", resolveEffectiveFontFamily("monospace"))
        assertEquals("monospace", resolveEffectiveFontFamily("Mono"))
        assertEquals("monospace", resolveEffectiveFontFamily("monospaced"))
        assertEquals("sans-serif", resolveEffectiveFontFamily("sans-serif"))
        assertEquals("sans-serif", resolveEffectiveFontFamily("sans serif"))
        assertEquals("serif", resolveEffectiveFontFamily("serif"))
    }

    @Test
    fun resolveEffectiveFontFamily_isCaseInsensitive() {
        assertEquals("monospace", resolveEffectiveFontFamily("MONOSPACE"))
        assertEquals("sans-serif", resolveEffectiveFontFamily("Sans-Serif"))
        assertEquals("serif", resolveEffectiveFontFamily("Serif"))
    }

    @Test
    fun resolveEffectiveFontFamily_trimsWhitespace() {
        assertEquals("monospace", resolveEffectiveFontFamily("  monospace  "))
        assertEquals("Roboto Mono", resolveEffectiveFontFamily("  Roboto Mono "))
    }

    @Test
    fun resolveEffectiveFontFamily_blankReturnsEmpty() {
        // K8: blank / whitespace-only input resolves to "" so the caller can fall
        // back to the system default instead of passing a meaningless family name.
        assertEquals("", resolveEffectiveFontFamily(""))
        assertEquals("", resolveEffectiveFontFamily("   "))
        assertEquals("", resolveEffectiveFontFamily("\t\n"))
    }
}
