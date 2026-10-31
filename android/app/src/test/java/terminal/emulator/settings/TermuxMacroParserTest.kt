package terminal.emulator.settings

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.input.KeyModifiers

/**
 * `extra-keys` 宏（`{macro: "CTRL f d"}`）的解析。
 *
 * 断言的是「写不出来/写错了会怎样」：宏是用户手写的外部输入，解析不出来时必须
 * 判为不可用（键不渲染），而不是渲染一个点了没反应的键。
 */
class TermuxMacroParserTest {

    @Test
    fun `a modifier applies to the next key only`() {
        val steps = MacroParser.parse("CTRL f d")
        assertEquals(
            listOf(
                MacroStep.Modifier(KeyModifiers.CTRL),
                MacroStep.Key(KeyEvent.KEYCODE_F, "f"),
                MacroStep.Key(KeyEvent.KEYCODE_D, "d"),
            ),
            steps,
        )
    }

    @Test
    fun `named keys and aliases resolve to key codes`() {
        val steps = MacroParser.parse("CTRL ESCAPE HOME")
        assertEquals(
            listOf(
                MacroStep.Modifier(KeyModifiers.CTRL),
                MacroStep.Key(KeyEvent.KEYCODE_ESCAPE, "ESCAPE"),
                MacroStep.Key(KeyEvent.KEYCODE_MOVE_HOME, "HOME"),
            ),
            steps,
        )
    }

    @Test
    fun `upper case letters carry shift and digits resolve to key codes`() {
        val steps = MacroParser.parse("A 1")
        assertEquals(
            listOf(
                MacroStep.Modifier(KeyModifiers.SHIFT),
                MacroStep.Key(KeyEvent.KEYCODE_A, "A"),
                MacroStep.Key(KeyEvent.KEYCODE_1, "1"),
            ),
            steps,
        )
    }

    @Test
    fun `shift is a modifier of the macro itself`() {
        val steps = MacroParser.parse("SHIFT f")
        assertEquals(
            listOf(
                MacroStep.Modifier(KeyModifiers.SHIFT),
                MacroStep.Key(KeyEvent.KEYCODE_F, "f"),
            ),
            steps,
        )
    }

    /** FN 没有映射：整个宏不可用，否则宏会静默少发一个键。 */
    @Test
    fun `a macro containing FN is unusable`() {
        assertNull(MacroParser.parse("FN f"))
    }

    @Test
    fun `tokens without a key code are literal text`() {
        val steps = MacroParser.parse("CTRL NOT_A_KEY é")
        assertEquals(
            listOf(
                MacroStep.Modifier(KeyModifiers.CTRL),
                MacroStep.Text("NOT_A_KEY"),
                MacroStep.Text("é"),
            ),
            steps,
        )
    }

    /** 空宏与只有修饰记号的宏都没有可发的东西。 */
    @Test
    fun `a macro without keys is unusable`() {
        assertNull(MacroParser.parse(""))
        assertNull(MacroParser.parse("CTRL ALT"))
    }

    /** 含 FN 的宏不可用：调用方据此不渲染该键。 */
    @Test
    fun `an unusable macro has no steps`() {
        assertTrue(MacroParser.parse("FN f").isNullOrEmpty())
    }

    /** 宏键帽默认逐记号套别名与字形，与上游 `ExtraKeyButton` 的显示文本一致。 */
    @Test
    fun `macro display applies the style to each token`() {
        assertEquals("CTRL F D", MacroParser.display("CTRL f d", ExtraKeysStyle.DEFAULT, true))
        assertEquals("⎈ F D", MacroParser.display("CTRL f d", ExtraKeysStyle.ALL, true))
        assertEquals("CTRL f d", MacroParser.display("CTRL f d", ExtraKeysStyle.DEFAULT, false))
    }

    /** 宏里的字面量按上游 `inputCodePoint` 折叠 Ctrl，并给每个码点加 Alt 的 ESC 前缀。 */
    @Test
    fun `literal macro text follows inputCodePoint`() {
        assertEquals(
            listOf<Byte>(6, 15, 15),
            MacroParser.encodeLiteral("foo", KeyModifiers.CTRL).toList(),
        )
        assertEquals(
            listOf<Byte>(27, 'a'.code.toByte(), 27, 'b'.code.toByte()),
            MacroParser.encodeLiteral("ab", KeyModifiers.ALT).toList(),
        )
        assertEquals(
            listOf<Byte>(27, 0xC3.toByte(), 0xA9.toByte()),
            MacroParser.encodeLiteral("é", KeyModifiers.ALT).toList(),
        )
        assertEquals(
            listOf<Byte>(1, 2),
            MacroParser.encodeLiteral("AB", KeyModifiers.CTRL).toList(),
        )
    }
}
