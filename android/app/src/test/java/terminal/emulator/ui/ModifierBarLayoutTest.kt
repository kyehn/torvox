package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import terminal.emulator.settings.ExtraKeysStyle
import terminal.emulator.settings.TerminalKeyCatalog
import terminal.emulator.settings.TermuxProperties

/** 固定布局 → 键栏槽位的转换（[buildExtraKeySlots]）。 */
class ModifierBarLayoutTest {

    /** DESIGN.md 修饰键栏节定死「固定 2 行 7 列」；默认布局就是这 14 个键。 */
    @Test
    fun `the default layout is two rows of seven`() {
        val slots = defaultExtraKeySlots()
        assertEquals(2, slots.size)
        assertEquals(7, slots[0].size)
        assertEquals(7, slots[1].size)
        assertEquals(TermuxProperties.DEFAULT_EXTRA_KEYS, slots.map { row -> row.map { it.name } })
    }

    @Test
    fun `unknown key names are dropped without dropping the row`() {
        val slots = buildExtraKeySlots(listOf(listOf("NOT_A_KEY", "ESC")), ExtraKeysStyle.DEFAULT, true)
        assertEquals(1, slots.size)
        assertEquals(listOf("ESC"), slots[0].map { it.name })
    }

    /** 单字符按键按字面量发送：没有键码，也没有特殊动作。 */
    @Test
    fun `single character keys are literals`() {
        val slot = buildExtraKeySlots(listOf(listOf("/")), ExtraKeysStyle.DEFAULT, true)[0][0]
        assertNull(slot.keyCode)
        assertNull(slot.special)
        assertEquals("/", slot.name)
    }

    /** 有键名的键取键码，且方向键标为可重复（按住连发）。 */
    @Test
    fun `named keys carry a key code and repetition flag`() {
        val slots = buildExtraKeySlots(listOf(listOf("UP", "ESC")), ExtraKeysStyle.DEFAULT, true)[0]
        assertEquals(true, slots[0].repeatable)
        assertEquals(false, slots[1].repeatable)
    }

    /** 特殊键（CTRL/ALT/…）不产生键码，交给键栏自身处理。 */
    @Test
    fun `special keys have no key code`() {
        val slot = buildExtraKeySlots(listOf(listOf("CTRL")), ExtraKeysStyle.DEFAULT, true)[0][0]
        assertNull(slot.keyCode)
        assertEquals(TerminalKeyCatalog.Special.CTRL, slot.special)
    }

    /**
     * `extra-keys-style` 只改显示字形，`testTag` 必须跨样式稳定。
     *
     * 测试与无障碍定位都按 testTag 找键；字形一变就找不到，等于把「换样式」变成
     * 「换一套控件」。
     */
    @Test
    fun `style changes the label but not the test tag`() {
        val layout = listOf(listOf("UP", "HOME"))
        val default = buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true)[0]
        val none = buildExtraKeySlots(layout, ExtraKeysStyle.NONE, true)[0]
        assertEquals(default.map { it.testTag }, none.map { it.testTag })
        assertEquals("↑", default[0].label)
        assertEquals("UP", none[0].label)
    }

    /** `extra-keys-text-all-caps = false` 时字母保持原样；符号永远不受影响。 */
    @Test
    fun `all caps only affects letters`() {
        val layout = listOf(listOf("Tab"))
        assertEquals("TAB", buildExtraKeySlots(layout, ExtraKeysStyle.ARROWS_ONLY, true)[0][0].label)
        assertEquals("Tab", buildExtraKeySlots(layout, ExtraKeysStyle.ARROWS_ONLY, false)[0][0].label)
        assertEquals(
            "⇱",
            buildExtraKeySlots(listOf(listOf("HOME")), ExtraKeysStyle.ARROWS_ALL, false)[0][0].label,
        )
    }

    /**
     * 五档样式是逐级加字形，不是五个随机外观：`default` 只换方向键与最常用的几个，
     * `arrows-all` 才加 Home/End/PgUp/PgDn，`all` 再加 ESC/CTRL/ALT，`none` 一个都不换。
     */
    @Test
    fun `style glyph sets follow the upstream ladder`() {
        fun label(name: String, style: ExtraKeysStyle) =
            buildExtraKeySlots(listOf(listOf(name)), style, true)[0][0].label

        assertEquals("↑", label("UP", ExtraKeysStyle.DEFAULT))
        assertEquals("↲", label("ENTER", ExtraKeysStyle.DEFAULT))
        assertEquals("HOME", label("HOME", ExtraKeysStyle.DEFAULT))
        assertEquals("⇱", label("HOME", ExtraKeysStyle.ARROWS_ALL))
        assertEquals("⎋", label("ESC", ExtraKeysStyle.ALL))
        assertEquals("ESC", label("ESC", ExtraKeysStyle.ARROWS_ALL))
        assertEquals("UP", label("UP", ExtraKeysStyle.NONE))
        assertEquals("ENTER", label("ENTER", ExtraKeysStyle.NONE))
    }
}
