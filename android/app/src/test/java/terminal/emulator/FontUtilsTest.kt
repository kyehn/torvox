package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Test

class FontUtilsTest {
    @Test
    fun `family names pass through untouched`() {
        // 字族名由外部库给出（DESIGN 字体选择节）：不得按别名手工改写，
        // 名字恰为 "Sans" 的字族曾被静默换成 "sans-serif"。
        assertEquals("JetBrains Mono", resolveEffectiveFontFamily("JetBrains Mono"))
        assertEquals("FiraCode Nerd Font", resolveEffectiveFontFamily("  FiraCode Nerd Font  "))
        assertEquals("Sans", resolveEffectiveFontFamily("Sans"))
        assertEquals("mono", resolveEffectiveFontFamily("mono"))
        assertEquals("monospaced", resolveEffectiveFontFamily("monospaced"))
        assertEquals("serif", resolveEffectiveFontFamily("serif"))
    }

    @Test
    fun `blank input returns empty`() {
        assertEquals("", resolveEffectiveFontFamily(""))
        assertEquals("", resolveEffectiveFontFamily("   "))
    }
}
