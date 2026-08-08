package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ToolbarMacroExpanderTest {
    @Test
    fun `single token is not a macro`() {
        assertFalse(ToolbarMacroExpander.isMacro("ESC"))
        assertFalse(ToolbarMacroExpander.isMacro(""))
        assertFalse(ToolbarMacroExpander.isMacro("   "))
    }

    @Test
    fun `multiple tokens or modifier tokens are macros`() {
        assertTrue(ToolbarMacroExpander.isMacro("CTRL c"))
        assertTrue(ToolbarMacroExpander.isMacro("CTRL ALT x"))
        assertTrue(ToolbarMacroExpander.isMacro("ESC q"))
    }

    @Test
    fun `empty and blank macro expand to nothing`() {
        assertEquals(emptyList<String>(), ToolbarMacroExpander.expand(""))
        assertEquals(emptyList<String>(), ToolbarMacroExpander.expand("  "))
    }

    @Test
    fun `ctrl letter folds to control byte`() {
        assertEquals(listOf("\u0003"), ToolbarMacroExpander.expand("CTRL c"))
        assertEquals(listOf("\u0001"), ToolbarMacroExpander.expand("CTRL a"))
        assertEquals(listOf("\u001A"), ToolbarMacroExpander.expand("CTRL z"))
        // Uppercase folds the same (termux inputCodePoint handles A-Z).
        assertEquals(listOf("\u0003"), ToolbarMacroExpander.expand("CTRL C"))
    }

    @Test
    fun `ctrl specials map per ANSI table`() {
        assertEquals(listOf("\u0000"), ToolbarMacroExpander.expand("CTRL 2"))
        assertEquals(listOf("\u001B"), ToolbarMacroExpander.expand("CTRL 3"))
        assertEquals(listOf("\u001C"), ToolbarMacroExpander.expand("CTRL 4"))
        assertEquals(listOf("\u001D"), ToolbarMacroExpander.expand("CTRL 5"))
        assertEquals(listOf("\u001E"), ToolbarMacroExpander.expand("CTRL 6"))
        assertEquals(listOf("\u001F"), ToolbarMacroExpander.expand("CTRL 7"))
        assertEquals(listOf("\u007F"), ToolbarMacroExpander.expand("CTRL 8"))
        assertEquals(listOf("\u001B"), ToolbarMacroExpander.expand("CTRL ["))
    }

    @Test
    fun `alt prefixes esc for text`() {
        assertEquals(listOf("\u001Bb"), ToolbarMacroExpander.expand("ALT b"))
        assertEquals(listOf("\u001Bx"), ToolbarMacroExpander.expand("ALT x"))
    }

    @Test
    fun `ctrl and alt combine`() {
        // termux: CTRL ALT x → ESC prefix + folded ctrl byte.
        assertEquals(listOf("\u001B\u0018"), ToolbarMacroExpander.expand("CTRL ALT x"))
    }

    @Test
    fun `modifiers are sticky for next key only`() {
        // CTRL applies to c, then resets: d is plain.
        assertEquals(
            listOf("\u0003", "d"),
            ToolbarMacroExpander.expand("CTRL c d"),
        )
    }

    @Test
    fun `modifier sequence applies to multiple keys`() {
        // CTRL c, CTRL d
        assertEquals(
            listOf("\u0003", "\u0004"),
            ToolbarMacroExpander.expand("CTRL c CTRL d"),
        )
    }

    @Test
    fun `special keys emit escape sequences`() {
        assertEquals(listOf("\u001b[A"), ToolbarMacroExpander.expand("UP"))
        assertEquals(listOf("\u001b[B"), ToolbarMacroExpander.expand("DOWN"))
        assertEquals(listOf("\u001b[H"), ToolbarMacroExpander.expand("HOME"))
        assertEquals(listOf("\u001b[F"), ToolbarMacroExpander.expand("END"))
        assertEquals(listOf("\u001b[5~"), ToolbarMacroExpander.expand("PGUP"))
        assertEquals(listOf("\u001b[6~"), ToolbarMacroExpander.expand("PGDN"))
        assertEquals(listOf("\u001b[3~"), ToolbarMacroExpander.expand("DEL"))
        assertEquals(listOf("\u001b[2~"), ToolbarMacroExpander.expand("INS"))
        assertEquals(listOf("\u007f"), ToolbarMacroExpander.expand("BKSP"))
        assertEquals(listOf("\t"), ToolbarMacroExpander.expand("TAB"))
        assertEquals(listOf("\r"), ToolbarMacroExpander.expand("ENTER"))
        assertEquals(listOf("\u001b"), ToolbarMacroExpander.expand("ESC"))
        assertEquals(listOf("\u001bOP"), ToolbarMacroExpander.expand("F1"))
        assertEquals(listOf("\u001b[24~"), ToolbarMacroExpander.expand("F12"))
    }

    @Test
    fun `aliases normalize alternate spellings`() {
        assertEquals(listOf("\u001b[5~"), ToolbarMacroExpander.expand("PAGEUP"))
        assertEquals(listOf("\u001b[5~"), ToolbarMacroExpander.expand("PAGE-UP"))
        assertEquals(listOf("\u001b[6~"), ToolbarMacroExpander.expand("PAGEDOWN"))
        assertEquals(listOf("\u001b[6~"), ToolbarMacroExpander.expand("PAGE_DOWN"))
        assertEquals(listOf("\u007f"), ToolbarMacroExpander.expand("BACKSPACE"))
        assertEquals(listOf("\u001b[3~"), ToolbarMacroExpander.expand("DELETE"))
        assertEquals(listOf("\\"), ToolbarMacroExpander.expand("BACKSLASH"))
        assertEquals(listOf("\""), ToolbarMacroExpander.expand("QUOTE"))
    }

    @Test
    fun `unknown tokens pass through verbatim`() {
        assertEquals(listOf("hello"), ToolbarMacroExpander.expand("hello"))
        assertEquals(listOf("|"), ToolbarMacroExpander.expand("|"))
    }

    @Test
    fun `modified special keys use csi modifier params`() {
        // termux transformForModifiers: SHIFT=2, ALT=3, CTRL=5.
        assertEquals(listOf("\u001b[1;5A"), ToolbarMacroExpander.expand("CTRL UP"))
        assertEquals(listOf("\u001b[1;3C"), ToolbarMacroExpander.expand("ALT RIGHT"))
        assertEquals(listOf("\u001b[1;2B"), ToolbarMacroExpander.expand("SHIFT DOWN"))
        assertEquals(listOf("\u001b[1;7D"), ToolbarMacroExpander.expand("CTRL ALT LEFT"))
    }

    @Test
    fun `alt tab and alt enter get esc prefix`() {
        assertEquals(listOf("\u001b\t"), ToolbarMacroExpander.expand("ALT TAB"))
        assertEquals(listOf("\u001b\r"), ToolbarMacroExpander.expand("ALT ENTER"))
    }

    @Test
    fun `fn is accepted but produces no output`() {
        // FN alone produces nothing (termux parity).
        assertEquals(emptyList<String>(), ToolbarMacroExpander.expand("FN"))
        // FN before a key: no output effect.
        assertEquals(listOf("x"), ToolbarMacroExpander.expand("FN x"))
    }

    @Test
    fun `combined macro example termux docs`() {
        // termux docs: "CTRL ALT C" = Ctrl+Alt+C → ESC + ETX.
        assertEquals(listOf("\u001b\u0003"), ToolbarMacroExpander.expand("CTRL ALT C"))
        // Multi-key: "ESC q" = ESC then q.
        assertEquals(listOf("\u001b", "q"), ToolbarMacroExpander.expand("ESC q"))
    }

    // ── round-227 T3 audit fix: full alias table (spec d3) ───────────────

    @Test
    fun `CONTROL alias behaves identically to CTRL`() {
        assertEquals(ToolbarMacroExpander.expand("CTRL a"), ToolbarMacroExpander.expand("CONTROL a"))
        assertEquals(listOf("\u0003"), ToolbarMacroExpander.expand("CONTROL c"))
    }

    @Test
    fun `ESCAPE SHFT RETURN FUNCTION aliases`() {
        assertEquals(listOf("\u001b"), ToolbarMacroExpander.expand("ESCAPE"))
        // SHFT + a is plain 'a' (shift is ignored for text, termux parity).
        assertEquals(listOf("a"), ToolbarMacroExpander.expand("SHFT a"))
        assertEquals(listOf("\r"), ToolbarMacroExpander.expand("RETURN"))
        // FUNCTION is accepted and produces no output (FN parity).
        assertEquals(emptyList<String>(), ToolbarMacroExpander.expand("FUNCTION"))
    }

    @Test
    fun `LT RT DN direction aliases`() {
        assertEquals(listOf("\u001b[D"), ToolbarMacroExpander.expand("LT"))
        assertEquals(listOf("\u001b[C"), ToolbarMacroExpander.expand("RT"))
        assertEquals(listOf("\u001b[B"), ToolbarMacroExpander.expand("DN"))
    }

    @Test
    fun `PAGE_UP and PAGE UP aliases`() {
        assertEquals(listOf("\u001b[5~"), ToolbarMacroExpander.expand("PAGE_UP"))
        assertEquals(listOf("\u001b[6~"), ToolbarMacroExpander.expand("PAGE_DOWN"))
        // "PAGE UP" contains a space, so the macro splitter yields two
        // tokens — identical to termux (its "PAGE UP" alias is likewise
        // unreachable after splitting): "PAGE" text + UP arrow.
        assertEquals(
            listOf("PAGE", "\u001b[A"),
            ToolbarMacroExpander.expand("PAGE UP"),
        )
    }

    @Test
    fun `double spaces do not consume sticky modifiers`() {
        // "CTRL  c" (double space) must equal "CTRL c": the empty token
        // must not release ctrl before 'c'.
        assertEquals(
            ToolbarMacroExpander.expand("CTRL c"),
            ToolbarMacroExpander.expand("CTRL  c"),
        )
        assertEquals(listOf("\u0003"), ToolbarMacroExpander.expand("CTRL  c"))
        assertTrue(ToolbarMacroExpander.isMacro("CTRL  c"))
    }

    @Test
    fun `leading and trailing spaces are ignored`() {
        assertEquals(listOf("\u0003"), ToolbarMacroExpander.expand("  CTRL c  "))
        assertEquals(emptyList<String>(), ToolbarMacroExpander.expand("  "))
    }

    // ── T3 edge-case additions: robustness ─────────────────────────────

    @Test
    fun `unknown modifier falls back to literal token`() {
        // "UNKNOWNKEY" is not a modifier or alias, so it emits as literal
        assertEquals(listOf("UNKNOWNKEY"), ToolbarMacroExpander.expand("UNKNOWNKEY"))
    }

    @Test
    fun `nested modifiers emit in order`() {
        // CTRL ALT x: x→ctrlFold=0x18(Ctrl-X), alt prefixes ESC
        val result = ToolbarMacroExpander.expand("CTRL ALT x")
        assertEquals(listOf("\u001b\u0018"), result)
    }

    @Test
    fun `f1 through f12 keys`() {
        // F1-F4 use SS3 (ESC O P/Q/R/S), F5+ use CSI numeric
        val f1 = ToolbarMacroExpander.expand("F1")
        assertEquals("F1 emits single token", 1, f1.size)
        assertEquals("F1 emits ESC O P", "\u001bOP", f1[0])
        val f10 = ToolbarMacroExpander.expand("F10")
        assertEquals("F10 emits single token", 1, f10.size)
        assertTrue("F10 starts with ESC[", f10[0].startsWith("\u001b["))
    }

    @Test
    fun `enter produces carriage return`() {
        assertEquals(listOf("\r"), ToolbarMacroExpander.expand("ENTER"))
    }

    @Test
    fun `backspace produces DEL`() {
        // BKSP → DEL (0x7F)
        assertEquals(listOf("\u007f"), ToolbarMacroExpander.expand("BKSP"))
    }

    @Test
    fun `escape produces ESC`() {
        assertEquals(listOf("\u001b"), ToolbarMacroExpander.expand("ESC"))
    }

    @Test
    fun `tab produces horizontal tab`() {
        assertEquals(listOf("\t"), ToolbarMacroExpander.expand("TAB"))
    }
}
