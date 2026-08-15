package terminal.emulator.shortcut

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests for [ShortcutBinding] serialization, deserialization, and key matching.
 *
 * Validates the serialize/deserialize round-trip and reserved-key guards.
 */
class ShortcutBindingTest {

    @Test
    fun `serialize Ctrl+Shift+V produces expected format`() {
        val binding = ShortcutBinding(key = KeyEvent.KEYCODE_V, ctrl = true, shift = true)
        assertEquals("CTRL|SHIFT|${KeyEvent.KEYCODE_V}", binding.serialize())
    }

    @Test
    fun `serialize plain key without modifiers`() {
        val binding = ShortcutBinding(key = KeyEvent.KEYCODE_ENTER)
        assertEquals("${KeyEvent.KEYCODE_ENTER}", binding.serialize())
    }

    @Test
    fun `serialize empty binding returns empty string`() {
        assertEquals("", ShortcutBinding.EMPTY.serialize())
    }

    @Test
    fun `deserialize round-trip for Ctrl+Shift+V`() {
        val original = ShortcutBinding(key = KeyEvent.KEYCODE_V, ctrl = true, shift = true)
        val deserialized = ShortcutBinding.deserialize(original.serialize())
        assertEquals(original, deserialized)
    }

    @Test
    fun `deserialize round-trip for plain key`() {
        val original = ShortcutBinding(key = KeyEvent.KEYCODE_A)
        val deserialized = ShortcutBinding.deserialize(original.serialize())
        assertEquals(original, deserialized)
    }

    @Test
    fun `deserialize round-trip for Ctrl+Alt+W`() {
        val original = ShortcutBinding(key = KeyEvent.KEYCODE_W, ctrl = true, alt = true)
        val deserialized = ShortcutBinding.deserialize(original.serialize())
        assertEquals(original, deserialized)
    }

    @Test
    fun `deserialize empty string returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize(""))
    }

    @Test
    fun `deserialize reserved HOME key returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("${KeyEvent.KEYCODE_HOME}"))
    }

    @Test
    fun `deserialize reserved BACK key returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("${KeyEvent.KEYCODE_BACK}"))
    }

    @Test
    fun `deserialize reserved volume key returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("${KeyEvent.KEYCODE_VOLUME_UP}"))
    }

    @Test
    fun `deserialize reserved modifier key returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("${KeyEvent.KEYCODE_CTRL_LEFT}"))
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("${KeyEvent.KEYCODE_SHIFT_LEFT}"))
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("${KeyEvent.KEYCODE_ALT_LEFT}"))
    }

    @Test
    fun `deserialize garbage string returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("not-a-number"))
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("|||"))
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("UNKNOWN_MOD|99"))
    }

    @Test
    fun `deserialize negative key code returns EMPTY`() {
        assertEquals(ShortcutBinding.EMPTY, ShortcutBinding.deserialize("CTRL|-1"))
    }

    @Test
    fun `isEmpty returns true only for EMPTY_KEY`() {
        assertTrue(ShortcutBinding.EMPTY.isEmpty())
        assertTrue(ShortcutBinding(ShortcutBinding.EMPTY_KEY).isEmpty())
        assertFalse(ShortcutBinding(key = KeyEvent.KEYCODE_A).isEmpty())
    }

    @Test
    fun `toDisplayString produces human readable label`() {
        val binding = ShortcutBinding(key = KeyEvent.KEYCODE_V, ctrl = true, shift = true)
        assertEquals("Ctrl+Shift+V", binding.toDisplayString())
    }

    @Test
    fun `toDisplayString for empty binding returns dash`() {
        assertEquals("\u2014", ShortcutBinding.EMPTY.toDisplayString())
    }

    @Test
    fun `defaults produces 5 bindings all non-empty`() {
        val defaults = KeyShortcutHandler.Defaults.all()
        assertEquals(5, defaults.size)
        defaults.values.forEach { binding ->
            assertFalse("Default binding should not be empty", binding.isEmpty())
        }
    }

    @Test
    fun `defaults uses expected action IDs`() {
        val defaults = KeyShortcutHandler.Defaults.all()
        assertTrue(defaults.containsKey("paste"))
        assertTrue(defaults.containsKey("new_session"))
        assertTrue(defaults.containsKey("close_session"))
        assertTrue(defaults.containsKey("copy"))
        assertTrue(defaults.containsKey("toggle_scroll"))
    }

    @Test
    fun `all defaults serialize and deserialize correctly`() {
        KeyShortcutHandler.Defaults.all().forEach { (actionId, binding) ->
            val roundTripped = ShortcutBinding.deserialize(binding.serialize())
            assertEquals("Round-trip failed for $actionId", binding, roundTripped)
        }
    }

    @Test
    fun `modifierFlagsMatch requires exact modifier combination`() {
        assertTrue(modifierFlagsMatch(KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON, ctrl = true, shift = true, alt = false, meta = false))
        assertFalse(modifierFlagsMatch(KeyEvent.META_CTRL_ON, ctrl = true, shift = true, alt = false, meta = false))
        assertFalse(modifierFlagsMatch(KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON, ctrl = true, shift = false, alt = false, meta = false))
    }

    @Test
    fun `modifierFlagsMatch accepts left and right modifier variants`() {
        // Left/right-specific mask bits (e.g. META_CTRL_LEFT_ON) are part of
        // the framework's META_*_MASK and must count as pressed.
        val leftCtrlShift = KeyEvent.META_CTRL_LEFT_ON or KeyEvent.META_SHIFT_LEFT_ON
        assertTrue(modifierFlagsMatch(leftCtrlShift, ctrl = true, shift = true, alt = false, meta = false))
        // Ctrl+Alt (no shift) must not match a Ctrl+Shift binding.
        val ctrlAlt = KeyEvent.META_CTRL_ON or KeyEvent.META_ALT_ON
        assertFalse(modifierFlagsMatch(ctrlAlt, ctrl = true, shift = true, alt = false, meta = false))
    }
}
