package terminal.emulator.input

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class KeyModifiersTest {
    @Test
    fun `fromStickyStates combines ctrl and alt`() {
        assertEquals(0, KeyModifiers.fromStickyStates(ModifierState.Off, ModifierState.Off))
        assertEquals(
            KeyModifiers.CTRL,
            KeyModifiers.fromStickyStates(ModifierState.Once, ModifierState.Off),
        )
        assertEquals(
            KeyModifiers.CTRL,
            KeyModifiers.fromStickyStates(ModifierState.Locked, ModifierState.Off),
        )
        assertEquals(
            KeyModifiers.ALT,
            KeyModifiers.fromStickyStates(ModifierState.Off, ModifierState.Locked),
        )
        assertEquals(
            KeyModifiers.CTRL or KeyModifiers.ALT,
            KeyModifiers.fromStickyStates(ModifierState.Locked, ModifierState.Locked),
        )
    }

    @Test
    fun `modifier state cycles off-once-locked`() {
        assertEquals(ModifierState.Once, ModifierState.Off.next())
        assertEquals(ModifierState.Locked, ModifierState.Once.next())
        assertEquals(ModifierState.Off, ModifierState.Locked.next())
        // Cycle is stable.
        assertEquals(ModifierState.Locked, ModifierState.Off.next().next())
    }

    @Test
    fun `tap toggle never locks (termux parity)`() {
        assertEquals(ModifierState.Once, ModifierState.Off.toggled())
        assertEquals(ModifierState.Off, ModifierState.Once.toggled())
        assertEquals(ModifierState.Off, ModifierState.Locked.toggled())
    }

    @Test
    fun `mask constants are distinct powers of two`() {
        assertNotEquals(KeyModifiers.SHIFT, KeyModifiers.ALT)
        assertNotEquals(KeyModifiers.ALT, KeyModifiers.CTRL)
        assertNotEquals(KeyModifiers.CTRL, KeyModifiers.META)
        assertEquals(1, KeyModifiers.SHIFT)
        assertEquals(2, KeyModifiers.ALT)
        assertEquals(4, KeyModifiers.CTRL)
        assertEquals(8, KeyModifiers.META)
    }

    @Test
    fun `ghosttyMods maps metaState to upstream bits`() {
        assertEquals(0, KeyModifiers.ghosttyMods(0))
        assertEquals(
            KeyModifiers.GhosttyMods.SHIFT,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_SHIFT_ON),
        )
        assertEquals(
            KeyModifiers.GhosttyMods.CTRL,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_CTRL_ON),
        )
        assertEquals(
            KeyModifiers.GhosttyMods.ALT,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_ALT_ON),
        )
        assertEquals(
            KeyModifiers.GhosttyMods.SUPER,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_META_ON),
        )
        assertEquals(
            KeyModifiers.GhosttyMods.SHIFT or KeyModifiers.GhosttyMods.CTRL,
            KeyModifiers.ghosttyMods(
                android.view.KeyEvent.META_SHIFT_ON or android.view.KeyEvent.META_CTRL_ON,
            ),
        )
    }
}
