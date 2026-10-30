package terminal.emulator.settings

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `~/.termux/colors.properties` 的解析与「覆盖槽位」的展开。
 *
 * 槽位数组是跨语言契约：19 个元素依次是背景、前景、光标、16 个 ANSI 色，未写的槽位
 * 填 `-1` 由原生取主题原色。错位一格不会报错，只会表现为「某些颜色不对」——所以这里
 * 逐个槽位比对，而不是只断言颜色数量。
 */
class ColorsPropertiesTest {

    private fun parse(text: String): Pair<ColorsProperties, List<String>> = ColorsPropertiesParser.parse(text)

    @Test
    fun `empty file covers nothing`() {
        val (colors, errors) = parse("")
        assertTrue(colors.isEmpty)
        assertTrue("空文件是正常状态，不该报错: $errors", errors.isEmpty())
    }

    @Test
    fun `primary colours are parsed`() {
        val (colors, errors) = parse(
            """
            foreground = #c0c0c0
            background = #101020
            cursor = #ff0000
            """.trimIndent(),
        )
        assertEquals(0xC0C0C0, colors.foreground)
        assertEquals(0x101020, colors.background)
        assertEquals(0xFF0000, colors.cursor)
        assertTrue(errors.isEmpty())
    }

    @Test
    fun `three digit shorthand expands per channel`() {
        val (colors, _) = parse("background = #f0a")
        assertEquals(0xFF00AA, colors.background)
    }

    /** `#AARRGGBB` 的 alpha 被丢弃：终端调色板不带透明度，保留它会让颜色算错。 */
    @Test
    fun `eight digit form keeps only rgb`() {
        val (colors, _) = parse("foreground = #80ff8000")
        assertEquals(0xFF8000, colors.foreground)
    }

    @Test
    fun `all sixteen ansi slots are read`() {
        val text = (0..15).joinToString("\n") { "color$it = #0000%02x".format(it) }
        val (colors, errors) = parse(text)
        assertEquals(16, colors.ansi.size)
        (0..15).forEach { assertEquals(it, colors.ansi[it]) }
        assertTrue(errors.isEmpty())
    }

    @Test
    fun `an out of range ansi slot is rejected and reported`() {
        val (colors, errors) = parse("color16 = #ffffff")
        assertTrue(colors.ansi.isEmpty())
        assertTrue("越界槽位必须留痕: $errors", errors.any { it.contains("color16") })
    }

    @Test
    fun `a malformed colour is ignored and reported`() {
        val (colors, errors) = parse("background = #12345")
        assertNull(colors.background)
        assertTrue(errors.any { it.contains("background") })
    }

    /** 一项写坏不影响其它项：外部配置文件不该因为一处笔误整份失效。 */
    @Test
    fun `one bad colour does not discard the rest`() {
        val (colors, errors) = parse(
            """
            background = not-a-colour
            cursor = #00ff00
            """.trimIndent(),
        )
        assertNull(colors.background)
        assertEquals(0x00FF00, colors.cursor)
        assertEquals(1, errors.size)
    }

    /**
     * 未写的槽位必须是 `-1`（不覆盖），而不是 `0`。
     *
     * `0` 是纯黑，落地成黑色背景/黑色光标——用户只改了一个 ANSI 色，整屏会变黑。
     */
    @Test
    fun `unset slots are marked as not overridden`() {
        val (colors, _) = parse("background = #101020")
        val slots = colors.toPaletteOverride()
        assertEquals(ColorsProperties.PALETTE_OVERRIDE_SLOTS, slots.size)
        assertEquals(0x101020, slots[0])
        (1 until slots.size).forEach { index ->
            assertEquals("槽位 $index 未写时必须是不覆盖", ColorsProperties.NO_OVERRIDE, slots[index])
        }
    }

    /** 19 个槽位逐个落到各自的位置。 */
    @Test
    fun `every slot lands in its own index`() {
        val text =
            buildString {
                appendLine("background = #010101")
                appendLine("foreground = #020202")
                appendLine("cursor = #030303")
                for (index in 0..15) {
                    appendLine("color$index = #%06x".format(0x100 + index))
                }
            }
        val (colors, _) = parse(text)
        val slots = colors.toPaletteOverride()
        assertArrayEquals(
            intArrayOf(
                0x010101,
                0x020202,
                0x030303,
                *IntArray(16) { 0x100 + it },
            ),
            slots,
        )
    }

    /** 空覆盖也要产出 19 个 `-1`：原生按长度校验，短数组会被拒收并抛异常。 */
    @Test
    fun `an empty override still has the full slot count`() {
        val slots = ColorsProperties().toPaletteOverride()
        assertEquals(ColorsProperties.PALETTE_OVERRIDE_SLOTS, slots.size)
        assertTrue(slots.all { it == ColorsProperties.NO_OVERRIDE })
    }

    /** 注释与空行被忽略。 */
    @Test
    fun `comments are ignored`() {
        val (colors, errors) = parse(
            """
            # a comment
            ! another
            cursor = #abcdef
            """.trimIndent(),
        )
        assertEquals(0xABCDEF, colors.cursor)
        assertTrue(errors.isEmpty())
    }
}
