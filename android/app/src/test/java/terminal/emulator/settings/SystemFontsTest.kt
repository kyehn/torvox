package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class SystemFontsTest {

    private val sampleXml =
        """
        <?xml version="1.0" encoding="utf-8"?>
        <familyset version="23">
            <family name="sans-serif">
                <font weight="400" style="normal">Roboto-Regular.ttf</font>
            </family>
            <family name="monospace">
                <font weight="400" style="normal">RobotoMono-Regular.ttf</font>
            </family>
            <family>
                <font weight="400" style="normal">Nameless.ttf</font>
            </family>
            <family name="">
                <font weight="400" style="normal">Empty.ttf</font>
            </family>
            <family name="serif">
                <font weight="400" style="normal">NotoSerif-Regular.ttf</font>
            </family>
        </familyset>
        """.trimIndent()

    @Test
    fun parse_returns_family_names_in_document_order() {
        assertEquals(listOf("sans-serif", "monospace", "serif"), parseFontsXmlFamilies(sampleXml))
    }

    @Test
    fun parse_skips_nameless_and_empty_names() {
        val names = parseFontsXmlFamilies(sampleXml)
        assertTrue(names.none { it.isEmpty() })
        assertEquals(3, names.size)
    }

    @Test
    fun parse_does_not_rewrite_names() {
        val xml = sampleXml.replace("monospace", "DroidSansMono")
        assertTrue(parseFontsXmlFamilies(xml).contains("DroidSansMono"))
    }

    @Test
    fun systemFonts_crashes_without_system_fonts_xml() {
        // No /system/etc/fonts.xml on the JVM: missing file must crash,
        // never silently fall back.
        assertThrows(IllegalStateException::class.java) { systemFonts() }
    }
}
