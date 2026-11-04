package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `~/.termux/colors.properties` 的解析。
 *
 * 这份文件是**外部输入**：用户手写、用 `adb push` 覆盖，或按 termux 的文档生成。
 * 断言的是键与色值的语义，以及坏键只作废自己——整份文件作废的症状是「改了一个颜色，
 * 终端全变样」，无从定位。
 */
class ColorsPropertiesParserTest {

    private fun parse(text: String): ColorsPropertiesLoad = ColorsPropertiesParser.parse(text)

    @Test
    fun `an empty file means no override`() {
        val loaded = parse("")
        assertTrue(loaded.properties.isEmpty)
        assertTrue("空文件是正常状态，不该报错: ${loaded.errors}", loaded.errors.isEmpty())
    }

    @Test
    fun `the three named colours are read`() {
        val loaded = parse(
            """
            foreground = #112233
            background = #445566
            cursor = #778899
            """.trimIndent(),
        )
        assertEquals(0x112233, loaded.properties.foreground)
        assertEquals(0x445566, loaded.properties.background)
        assertEquals(0x778899, loaded.properties.cursor)
    }

    @Test
    fun `the sixteen ansi slots are read`() {
        val loaded = parse("color0 = #000001\ncolor15 = #00000f")
        assertEquals(0x000001, loaded.properties.ansi[0])
        assertEquals(0x00000f, loaded.properties.ansi[15])
        assertEquals(2, loaded.properties.ansi.size)
    }

    /**
     * 256 色立方不在本应用的调色板里：16..255 由 `libghostty-vt` 按 xterm 标准生成，
     * 覆盖它们没有去处，只能明确拒绝并说明。
     */
    @Test
    fun `the 256 colour cube slots are rejected`() {
        val loaded = parse("color16 = #000000")
        assertTrue(loaded.properties.ansi.isEmpty())
        assertEquals(1, loaded.errors.size)
        assertTrue("错误须点名不支持的键: ${loaded.errors}", loaded.errors[0].contains("color16"))
    }

    /** 上游把 `color256`/`color257`/`color258` 写进与三个具名键相同的下标，是它们的别名。 */
    @Test
    fun `the upstream aliases are accepted`() {
        val loaded = parse("color256 = #111111\ncolor257 = #222222\ncolor258 = #333333")
        assertEquals(0x111111, loaded.properties.foreground)
        assertEquals(0x222222, loaded.properties.background)
        assertEquals(0x333333, loaded.properties.cursor)
    }

    /**
     * 显式键优先于别名：上游按 `Properties` 的遍历顺序覆盖，两种写法并存时结果取决于
     * 哈希顺序，本仓不能让结果随机。
     */
    @Test
    fun `an explicit key wins over its alias`() {
        val loaded = parse("foreground = #ffffff\ncolor256 = #000000")
        assertEquals(0xffffff, loaded.properties.foreground)
    }

    /** 色值语法照上游 `TerminalColors.parse`：XQueryColor 的几种宽度，外加 `rgb:` 记法。 */
    @Test
    fun `the colour syntax follows XQueryColor`() {
        assertEquals(0xff0000, parseXQueryColor("#f00"))
        assertEquals(0xff0000, parseXQueryColor("#FF0000"))
        assertEquals(0xff0000, parseXQueryColor("#fff000000"))
        assertEquals(0xff0000, parseXQueryColor("#ffff00000000"))
        assertEquals(0x00ff00, parseXQueryColor("rgb:00/FF/00"))
    }

    @Test
    fun `a malformed colour is rejected`() {
        assertNull(parseXQueryColor("not-a-color"))
        assertNull(parseXQueryColor("#ff00"))
        assertNull(parseXQueryColor("#12345"))
    }

    /** 坏色值只作废自己那项：其余键照常生效。 */
    @Test
    fun `an invalid colour only drops its own key`() {
        val loaded = parse("background = not-a-color\ncursor = #00FF00")
        assertNull(loaded.properties.background)
        assertEquals(0x00ff00, loaded.properties.cursor)
        assertEquals(1, loaded.errors.size)
    }

    @Test
    fun `an unknown key is ignored`() {
        val loaded = parse("font-size = 12\nbackground = #101010")
        assertEquals(0x101010, loaded.properties.background)
        assertTrue("未知键不该报错: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /**
     * 背景被覆盖而光标未指定时，光标按背景亮度定（上游
     * `setCursorColorForBackground`）：主题自带的光标色是按主题背景算的，背景换掉后
     * 它可能与新背景同色而看不见。
     */
    @Test
    fun `the cursor follows the overridden background`() {
        assertEquals(0x000000, parse("background = #FFFFFF").properties.cursor)
        assertEquals(0xffffff, parse("background = #101010").properties.cursor)
    }

    @Test
    fun `an explicit cursor wins over the background derived one`() {
        assertEquals(0xff0000, parse("background = #FFFFFF\ncursor = #FF0000").properties.cursor)
    }

    /** 背景没被覆盖时不动主题自带的光标色。 */
    @Test
    fun `the cursor is untouched when the background is not overridden`() {
        assertNull(parse("foreground = #FFFFFF").properties.cursor)
    }

    /**
     * 覆盖通道的槽位顺序是跨语言契约：背景、前景、光标，随后 16 个 ANSI 色；
     * 未覆盖的槽填 `-1`，由原生取主题原色。
     */
    @Test
    fun `the palette override keeps the slot order`() {
        val slots = parse("background = #102030\nforeground = #405060\ncursor = #708090\ncolor1 = #AABBCC")
            .properties.toPaletteOverride()
        assertEquals(ColorsProperties.PALETTE_OVERRIDE_SLOTS, slots.size)
        assertEquals(0x102030, slots[0])
        assertEquals(0x405060, slots[1])
        assertEquals(0x708090, slots[2])
        assertEquals(ColorsProperties.NO_OVERRIDE, slots[3])
        assertEquals(0xaabbcc, slots[4])
        assertEquals(ColorsProperties.NO_OVERRIDE, slots[5])
    }
}
