package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class NerdKeyLabelsTest {
    @Test
    fun `maps special keys to nerd font glyphs`() {
        assertEquals("\uEE59", NerdKeyLabels.label("ESC"))
        assertEquals("\uEB8A", NerdKeyLabels.label("TAB"))
        assertEquals("\uEB90", NerdKeyLabels.label("HOME"))
        assertEquals("\uEB94", NerdKeyLabels.label("END"))
        assertEquals("\uEB96", NerdKeyLabels.label("PGUP"))
        assertEquals("\uEB95", NerdKeyLabels.label("PGDN"))
    }

    @Test
    fun `scroll label is a surrogate pair`() {
        val label = NerdKeyLabels.label("SCROLL")
        assertEquals(2, label.length)
        assertEquals(0xDB81, label[0].code)
        assertEquals(0xDC0E, label[1].code)
    }

    @Test
    fun `ctrl and alt stay literal`() {
        assertEquals("CTRL", NerdKeyLabels.label("CTRL"))
        assertEquals("ALT", NerdKeyLabels.label("ALT"))
    }

    @Test
    fun `unknown key falls back to itself`() {
        assertEquals("SPACE", NerdKeyLabels.label("SPACE"))
    }
}
