package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ExtraKeysParserTest {

    @Test
    fun `parse empty string returns empty`() {
        assertEquals(emptyList<ToolbarItem>(), ExtraKeysParser.parse(""))
    }

    @Test
    fun `parse blank string returns empty`() {
        assertEquals(emptyList<ToolbarItem>(), ExtraKeysParser.parse("   "))
    }

    @Test
    fun `parse null-ish string returns empty`() {
        assertEquals(emptyList<ToolbarItem>(), ExtraKeysParser.parse(""))
    }

    @Test
    fun `parse simple keys`() {
        val result = ExtraKeysParser.parse("ESC TAB CTRL")
        assertEquals(3, result.size)
        assertTrue(result[0] is ToolbarItem.Default)
        assertTrue(result[1] is ToolbarItem.Default)
        assertTrue(result[2] is ToolbarItem.Default)
        assertEquals(ToolbarKey.ESC, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.TAB, (result[1] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.CTRL, (result[2] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse with columns`() {
        val result = ExtraKeysParser.parse("ESC | TAB | CTRL")
        assertEquals(3, result.size)
    }

    @Test
    fun `parse sticky group`() {
        val result = ExtraKeysParser.parse("[ESC TAB]")
        assertEquals(2, result.size)
        assertTrue(result[0] is ToolbarItem.Default)
        assertTrue(result[1] is ToolbarItem.Default)
        assertEquals(ToolbarKey.ESC, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.TAB, (result[1] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse mixed sticky and regular columns`() {
        // [ESC TAB] | [CTRL ALT] | ESC TAB
        val result = ExtraKeysParser.parse("[ESC TAB] | [CTRL ALT] | ESC TAB")
        assertEquals(6, result.size)
        // First column: ESC, TAB (sticky)
        assertEquals(ToolbarKey.ESC, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.TAB, (result[1] as ToolbarItem.Default).key)
        // Second column: CTRL, ALT (sticky)
        assertEquals(ToolbarKey.CTRL, (result[2] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.ALT, (result[3] as ToolbarItem.Default).key)
        // Third column: ESC, TAB (regular)
        assertEquals(ToolbarKey.ESC, (result[4] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.TAB, (result[5] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse unknown keys become custom`() {
        val result = ExtraKeysParser.parse("ENTER")
        assertEquals(1, result.size)
        assertTrue(result[0] is ToolbarItem.Custom)
        assertEquals("ENTER", (result[0] as ToolbarItem.Custom).label)
    }

    @Test
    fun `parse termux arrow alias`() {
        // UP → ARROW_UP
        val result = ExtraKeysParser.parse("UP DOWN LEFT RIGHT")
        assertEquals(4, result.size)
        assertEquals(ToolbarKey.ARROW_UP, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.ARROW_DOWN, (result[1] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.ARROW_LEFT, (result[2] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.ARROW_RIGHT, (result[3] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse PAGEUP alias`() {
        val result = ExtraKeysParser.parse("PAGEUP PAGEDOWN")
        assertEquals(2, result.size)
        assertEquals(ToolbarKey.PGUP, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.PGDN, (result[1] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse PGUP directly`() {
        val result = ExtraKeysParser.parse("PGUP PGDN")
        assertEquals(2, result.size)
        assertEquals(ToolbarKey.PGUP, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.PGDN, (result[1] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse ENTER produces custom with carriage return sequence`() {
        val result = ExtraKeysParser.parse("ENTER")
        assertEquals(1, result.size)
        val item = result[0] as ToolbarItem.Custom
        assertEquals("ENTER", item.label)
        assertEquals("\r", item.sequence)
    }

    @Test
    fun `parse F1 produces custom with escape sequence`() {
        val result = ExtraKeysParser.parse("F1 F12")
        assertEquals(2, result.size)
        val f1 = result[0] as ToolbarItem.Custom
        assertEquals("F1", f1.label)
        assertEquals("\u001bOP", f1.sequence)
        val f12 = result[1] as ToolbarItem.Custom
        assertEquals("\u001b[24~", f12.sequence)
    }

    @Test
    fun `parse BKSP produces custom with DEL sequence`() {
        val result = ExtraKeysParser.parse("BKSP")
        assertEquals(1, result.size)
        val item = result[0] as ToolbarItem.Custom
        assertEquals("\u007f", item.sequence)
    }

    @Test
    fun `parse BACKSPACE alias`() {
        val result = ExtraKeysParser.parse("BACKSPACE")
        assertEquals(1, result.size)
        val item = result[0] as ToolbarItem.Custom
        assertEquals("\u007f", item.sequence)
    }

    @Test
    fun `parse extra spaces are trimmed`() {
        val result = ExtraKeysParser.parse("  ESC   TAB  ")
        assertEquals(2, result.size)
        assertEquals(ToolbarKey.ESC, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.TAB, (result[1] as ToolbarItem.Default).key)
    }

    @Test
    fun `parse multiple sticky groups`() {
        val result = ExtraKeysParser.parse("[CTRL ALT] | [ESC]")
        assertEquals(3, result.size)
        assertEquals(ToolbarKey.CTRL, (result[0] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.ALT, (result[1] as ToolbarItem.Default).key)
        assertEquals(ToolbarKey.ESC, (result[2] as ToolbarItem.Default).key)
    }

    @Test
    fun `export roundtrip`() {
        val items = listOf(
            ToolbarItem.Default(ToolbarKey.ESC),
            ToolbarItem.Default(ToolbarKey.TAB),
        )
        val exported = ExtraKeysParser.export(items)
        assertTrue(exported.contains("ESC"))
        assertTrue(exported.contains("TAB"))
        assertTrue(exported.contains(" | "))
    }

    @Test
    fun `export uses enum name for default keys`() {
        val items = listOf(ToolbarItem.Default(ToolbarKey.ARROW_UP))
        assertEquals("ARROW_UP", ExtraKeysParser.export(items))
    }

    @Test
    fun `export uses label for custom keys`() {
        val items = listOf(ToolbarItem.Custom(label = "F1", sequence = "\u001bOP"))
        assertEquals("F1", ExtraKeysParser.export(items))
    }

    @Test
    fun `export single item has no separator`() {
        val items = listOf(ToolbarItem.Default(ToolbarKey.ESC))
        assertEquals("ESC", ExtraKeysParser.export(items))
    }

    @Test
    fun `custom key has expected id prefix`() {
        val result = ExtraKeysParser.parse("ENTER")
        val item = result[0] as ToolbarItem.Custom
        assertTrue(item.id.startsWith("extra_"))
    }
}
