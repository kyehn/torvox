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
    fun `mask constants are distinct powers of two`() {
        assertNotEquals(KeyModifiers.SHIFT, KeyModifiers.ALT)
        assertNotEquals(KeyModifiers.ALT, KeyModifiers.CTRL)
        assertNotEquals(KeyModifiers.CTRL, KeyModifiers.META)
        assertEquals(1, KeyModifiers.SHIFT)
        assertEquals(2, KeyModifiers.ALT)
        assertEquals(4, KeyModifiers.CTRL)
        assertEquals(8, KeyModifiers.META)
    }
}
