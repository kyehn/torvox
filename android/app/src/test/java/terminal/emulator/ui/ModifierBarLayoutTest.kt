package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.settings.ExtraKey
import terminal.emulator.settings.ExtraKeysStyle
import terminal.emulator.settings.TerminalKeyCatalog
import terminal.emulator.settings.TermuxProperties

/**
 * `extra-keys` → 键栏槽位的转换（[buildExtraKeySlots] / [extraKeyRowCount]）。
 *
 * 两个函数必须给出**同一个行数**：前者决定画几行，后者决定网格为键栏预留多少高度。
 * 一旦分家，键栏就会盖住终端底部若干行——它盖在 Surface 之上，被盖的行既看不见也
 * 点不到，而日志里没有任何异常。
 */
class ModifierBarLayoutTest {

    /** `DESIGN.md:157` 定死「固定 2 行 7 列」；默认布局就是这 14 个键，全部可渲染。 */
    @Test
    fun `the default layout is two rows of seven`() {
        val slots = defaultExtraKeySlots()
        assertEquals(2, slots.size)
        assertEquals(7, slots[0].size)
        assertEquals(7, slots[1].size)
    }

    @Test
    fun `row count matches what is actually rendered`() {
        val layout = TermuxProperties.DEFAULT_EXTRA_KEYS
        assertEquals(buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true).size, extraKeyRowCount(layout))
    }

    /**
     * 只写了 `FN` 的一行不占高度。
     *
     * FN 需要一整套二次映射（未实现），故不渲染；若预留高度仍按原始行数算，键栏下方
     * 会空出一条终端的空白带。
     */
    @Test
    fun `a row of only unsupported keys does not reserve height`() {
        val layout = listOf(listOf(ExtraKey("FN")), listOf(ExtraKey("ESC")))
        assertEquals(1, extraKeyRowCount(layout))
        assertEquals(1, buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true).size)
    }

    @Test
    fun `unknown key names are dropped`() {
        val layout = listOf(listOf(ExtraKey("NOT_A_KEY"), ExtraKey("ESC")))
        val slots = buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true)
        assertEquals(1, slots.size)
        assertEquals(listOf("ESC"), slots[0].map { it.name })
        assertEquals(1, extraKeyRowCount(layout))
    }

    /** 单字符按键按字面量发送：没有键码，也没有特殊动作。 */
    @Test
    fun `single character keys are literals`() {
        val slot = buildExtraKeySlots(listOf(listOf(ExtraKey("/"))), ExtraKeysStyle.DEFAULT, true)[0][0]
        assertNull(slot.keyCode)
        assertNull(slot.special)
        assertEquals("/", slot.name)
    }

    /** 有键名的键取键码，且方向键标为可重复（按住连发）。 */
    @Test
    fun `named keys carry a key code and repetition flag`() {
        val slots = buildExtraKeySlots(listOf(listOf(ExtraKey("UP"), ExtraKey("ESC"))), ExtraKeysStyle.DEFAULT, true)[0]
        assertEquals(true, slots[0].repeatable)
        assertEquals(false, slots[1].repeatable)
    }

    /** 特殊键（CTRL/ALT/…）不产生键码，交给键栏自身处理。 */
    @Test
    fun `special keys have no key code`() {
        val slot = buildExtraKeySlots(listOf(listOf(ExtraKey("CTRL"))), ExtraKeysStyle.DEFAULT, true)[0][0]
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
        val layout = listOf(listOf(ExtraKey("UP"), ExtraKey("HOME")))
        val default = buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true)[0]
        val none = buildExtraKeySlots(layout, ExtraKeysStyle.NONE, true)[0]
        assertEquals(default.map { it.testTag }, none.map { it.testTag })
        assertEquals("↑", default[0].label)
        assertEquals("UP", none[0].label)
    }

    /** `extra-keys-text-all-caps = false` 时字母保持原样；符号永远不受影响。 */
    @Test
    fun `all caps only affects letters`() {
        val layout = listOf(listOf(ExtraKey("Tab")))
        assertEquals("TAB", buildExtraKeySlots(layout, ExtraKeysStyle.ARROWS_ONLY, true)[0][0].label)
        assertEquals("Tab", buildExtraKeySlots(layout, ExtraKeysStyle.ARROWS_ONLY, false)[0][0].label)
        // 有字形的键在两种取值下都一样。
        assertEquals(
            "⇱",
            buildExtraKeySlots(listOf(listOf(ExtraKey("HOME"))), ExtraKeysStyle.ARROWS_ALL, false)[0][0].label,
        )
    }

    /**
     * 五档样式是逐级加字形，不是五个随机外观：`default` 只换方向键与最常用的几个，
     * `arrows-all` 才加 Home/End/PgUp/PgDn，`all` 再加 ESC/CTRL/ALT，`none` 一个都不换。
     */
    @Test
    fun `style glyph sets follow the upstream ladder`() {
        fun label(name: String, style: ExtraKeysStyle) =
            buildExtraKeySlots(listOf(listOf(ExtraKey(name))), style, true)[0][0].label

        // default：方向键与常用符号有字形，Home 仍是文字。
        assertEquals("↑", label("UP", ExtraKeysStyle.DEFAULT))
        assertEquals("↲", label("ENTER", ExtraKeysStyle.DEFAULT))
        assertEquals("HOME", label("HOME", ExtraKeysStyle.DEFAULT))
        // arrows-all：Home/End 也有字形了。
        assertEquals("⇱", label("HOME", ExtraKeysStyle.ARROWS_ALL))
        // all：ESC 等换成 ISO 符号。
        assertEquals("⎋", label("ESC", ExtraKeysStyle.ALL))
        assertEquals("ESC", label("ESC", ExtraKeysStyle.ARROWS_ALL))
        // none：全部保持文字。
        assertEquals("UP", label("UP", ExtraKeysStyle.NONE))
        assertEquals("ENTER", label("ENTER", ExtraKeysStyle.NONE))
    }

    /** popup 随槽位带下去，供长按发送。 */
    @Test
    fun `popup text survives the conversion`() {
        val slot = buildExtraKeySlots(listOf(listOf(ExtraKey("-", popup = "|"))), ExtraKeysStyle.DEFAULT, true)[0][0]
        assertEquals("|", slot.popup)
    }

    /** 空行不占高度（termux 的解析器同样不接受空行）。 */
    @Test
    fun `empty rows are dropped`() {
        val layout = listOf(emptyList(), listOf(ExtraKey("ESC")))
        assertEquals(1, extraKeyRowCount(layout))
        assertEquals(1, buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true).size)
    }

    /** 自定义布局的行数逐行照搬，不多不少。 */
    @Test
    fun `multi row layouts keep their row count`() {
        val layout =
            listOf(
                listOf(ExtraKey("ESC"), ExtraKey("TAB")),
                listOf(ExtraKey("HOME")),
                listOf(ExtraKey("LEFT"), ExtraKey("DOWN"), ExtraKey("RIGHT")),
            )
        assertEquals(3, extraKeyRowCount(layout))
        val slots = buildExtraKeySlots(layout, ExtraKeysStyle.DEFAULT, true)
        assertEquals(3, slots.size)
        assertTrue(slots.all { it.isNotEmpty() })
    }
}
