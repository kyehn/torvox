package terminal.emulator.input

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

class DeadKeyStateTest {
    /** 把组合键状态机与平台键位表解耦：只测状态与派生值，不依赖真实布局。 */
    private fun state(compose: (Int, Int) -> Int = { _, base -> base }) = DeadKeyState(compose)

    /** `getUnicodeChar` 对死键返回的组合符 + 高位标志，与平台同一形状。 */
    private fun dead(value: Int) = value or 0x80000000.toInt()

    @Test
    fun `a dead key is recognized by its high bit`() {
        assertTrue(state().isDead(dead('`'.code)))
        assertTrue(state().isDead(dead('^'.code)))
        // 可打印字符与 0 都不带标志位。
        assertFalse(state().isDead('e'.code))
        assertFalse(state().isDead(0))
    }

    @Test
    fun `holding a dead key strips the flag and keeps the accent`() {
        val state = state()
        state.hold(dead('`'.code))
        assertEquals('`'.code, state.accent)
    }

    @Test
    fun `a second dead key replaces the first`() {
        val state = state()
        state.hold(dead('`'.code))
        state.hold(dead('^'.code))
        // 前者没有可合成的对象，以最后一个为准。
        assertEquals('^'.code, state.accent)
    }

    @Test
    fun `take hands the accent out and disarms it`() {
        val state = state()
        state.hold(dead('`'.code))
        assertEquals('`'.code, state.take())
        assertEquals(0, state.accent)
    }

    @Test
    fun `accent composes only with the key that follows it`() {
        val state = state({ _, _ -> 0x00e8 }) // 合成的 è
        state.hold(dead('`'.code))
        val accent = state.take()
        assertEquals(0x00e8, state.composeWith(accent, 'e'.code))
        // 组合符已随 take 作废：再调合成函数也不得凭空产出组合字符。
        assertEquals(0, state.composeWith(0, 'a'.code))
    }

    @Test
    fun `compose returns zero when the layout has no such combination`() {
        val state = state({ _, _ -> 0 })
        state.hold(dead('`'.code))
        assertEquals(0, state.composeWith(state.take(), 'q'.code))
    }

    @Test
    fun `no pending accent composes nothing`() {
        assertEquals(0, state().composeWith(0, 'e'.code))
    }
}

/**
 * 默认合成器就是 `KeyCharacterMap::getDeadChar`。状态机单测全部注入假函数，看不到
 * 这一层，故单独在真实框架上调一次：方法引用若在运行时不可用，设备上会直接崩。
 *
 * 顺带钉住组合结果的真实值：Robolectric 默认键位表把重音符 + e 合成为 `è`（232）。
 */
@RunWith(RobolectricTestRunner::class)
class DeadKeyStatePlatformTest {
    @Test
    fun `default composer reaches the platform key map`() {
        val state = DeadKeyState()
        state.hold(0x80000000.toInt() or '`'.code)
        assertEquals(232, state.composeWith(state.take(), 'e'.code))
    }
}

/** 左右 Alt 的区分要读 KeyEvent，故同上需要真实框架。 */
@RunWith(RobolectricTestRunner::class)
class KeyModifiersFromKeyEventTest {
    private fun maskOf(vararg modifiers: Int) = KeyModifiers.fromKeyEvent(
        KeyEvent(0, 0, KeyEvent.ACTION_DOWN, 0, 0, modifiers.fold(0) { acc, m -> acc or m }, 0, 0),
        ModifierState.Off,
        ModifierState.Off,
    )

    @Test
    fun `left alt sets ALT`() {
        assertEquals(KeyModifiers.ALT, maskOf(KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON))
    }

    @Test
    fun `right alt (AltGr) sets no ALT`() {
        // AltGr 只作布局的字符级修饰：置了 ALT 位会让编码器发 ESC 前缀。
        assertEquals(0, maskOf(KeyEvent.META_ALT_ON or KeyEvent.META_ALT_RIGHT_ON))
    }
}

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
    fun `only once and locked count as an active modifier`() {
        // 一次性与锁定都表示「此刻按住」；关闭态不算。
        assertFalse(ModifierState.Off.isActive())
        assertTrue(ModifierState.Once.isActive())
        assertTrue(ModifierState.Locked.isActive())
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
