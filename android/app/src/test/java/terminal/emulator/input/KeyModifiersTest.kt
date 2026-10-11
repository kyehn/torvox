package terminal.emulator.input

import org.junit.Assert.assertEquals
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
    fun `mask constants are the bits the native side decodes`() {
        // 掩码要跨 JNI 送到原生去解码，故位序本身是对外契约：改成别的位即红。
        // 位值即上游 `key.Mods` 的原始位——全仓唯一一种修饰表示。
        // 「互不相同」由下面四条具体取值蕴含，不再另立断言。
        assertEquals(1, KeyModifiers.SHIFT)
        assertEquals(2, KeyModifiers.CTRL)
        assertEquals(4, KeyModifiers.ALT)
        assertEquals(8, KeyModifiers.SUPER)
    }

    @Test
    fun `ghosttyMods maps metaState to the same bits`() {
        assertEquals(0, KeyModifiers.ghosttyMods(0))
        assertEquals(
            KeyModifiers.SHIFT,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_SHIFT_ON),
        )
        assertEquals(
            KeyModifiers.CTRL,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_CTRL_ON),
        )
        assertEquals(
            KeyModifiers.ALT,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_ALT_ON),
        )
        assertEquals(
            KeyModifiers.SUPER,
            KeyModifiers.ghosttyMods(android.view.KeyEvent.META_META_ON),
        )
        assertEquals(
            KeyModifiers.SHIFT or KeyModifiers.CTRL,
            KeyModifiers.ghosttyMods(
                android.view.KeyEvent.META_SHIFT_ON or android.view.KeyEvent.META_CTRL_ON,
            ),
        )
    }
}
