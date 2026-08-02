package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Test

class FontUtilsTest {
    @Test
    fun `monospace aliases normalize to monospace`() {
        assertEquals("monospace", resolveEffectiveFontFamily("monospace"))
        assertEquals("monospace", resolveEffectiveFontFamily("mono"))
        assertEquals("monospace", resolveEffectiveFontFamily("monospaced"))
        assertEquals("monospace", resolveEffectiveFontFamily("  MONOSPACE  "))
    }

    @Test
    fun `sans aliases normalize to sans-serif`() {
        assertEquals("sans-serif", resolveEffectiveFontFamily("sans-serif"))
        assertEquals("sans-serif", resolveEffectiveFontFamily("sans"))
        assertEquals("sans-serif", resolveEffectiveFontFamily("sans serif"))
        assertEquals("sans-serif", resolveEffectiveFontFamily("SANS-SERIF"))
    }

    @Test
    fun `serif passes through`() {
        assertEquals("serif", resolveEffectiveFontFamily("serif"))
    }

    @Test
    fun `named families pass through untouched`() {
        assertEquals("JetBrains Mono", resolveEffectiveFontFamily("JetBrains Mono"))
        assertEquals("FiraCode Nerd Font", resolveEffectiveFontFamily("  FiraCode Nerd Font  "))
    }

    @Test
    fun `blank input returns empty`() {
        assertEquals("", resolveEffectiveFontFamily(""))
        assertEquals("", resolveEffectiveFontFamily("   "))
    }
}
