package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `~/.termux/termux.properties` 的解析。
 *
 * 断言的是「写错了会怎样」而不只是「写对了会读成什么」：这份文件是**外部输入**，
 * 用户手写、可被 `adb push` 覆盖、也会被别的应用按 termux 的文档生成。一个写错的键
 * 必须只影响它自己那项（保留默认 + 记错误），不能把整份配置作废。
 */
class TermuxPropertiesParserTest {

    private fun parse(text: String): TermuxPropertiesLoad = TermuxPropertiesParser.parse(text)

    @Test
    fun `empty file means every default`() {
        val loaded = parse("")
        assertEquals(TermuxProperties(), loaded.properties)
        assertTrue("空文件是正常状态，不该报错: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /** 启动不主动弹键盘（与既有外观一致，上游默认相反）。 */
    @Test
    fun `keyboard startup default matches the app's behaviour`() {
        assertTrue(parse("").properties.hideSoftKeyboardOnStartup)
    }

    @Test
    fun `booleans are read case-insensitively`() {
        val loaded = parse(
            """
            fullscreen = TRUE
            disable-hardware-keyboard-shortcuts = True
            ctrl-space-workaround=true
            """.trimIndent(),
        )
        assertTrue(loaded.properties.fullscreen)
        assertTrue(loaded.properties.disableHardwareKeyboardShortcuts)
        assertTrue(loaded.properties.ctrlSpaceWorkaround)
    }

    /**
     * 非法布尔值只影响该项：其余键照常生效。
     *
     * 若实现成「遇到非法值就整份作废」，用户改错一个 `fullscreen` 会让另外二十项一起
     * 回退默认，症状是「改了这个那个也坏了」，极难定位。
     */
    @Test
    fun `an invalid boolean keeps its default and leaves the rest alone`() {
        val loaded = parse(
            """
            fullscreen = yes-please
            terminal-transcript-rows = 1000
            """.trimIndent(),
        )
        assertEquals(false, loaded.properties.fullscreen)
        assertEquals(1000, loaded.properties.terminalTranscriptRows)
        assertTrue(
            "非法值必须留下可读的错误: ${loaded.errors}",
            loaded.errors.any { it.contains("fullscreen") },
        )
    }

    /**
     * 越界值回退默认，不钳到边界（上游 `getDefaultIfNotInRange` 的口径）。
     *
     * 钳制会让「写成 99999」变成 50000 行回滚——用户以为自己配了 10 万行，实际拿到
     * 的是 5 万；回退默认则与上游、与本仓 `config.md` 一致。
     */
    @Test
    fun `margins outside the range fall back to the default`() {
        assertEquals(3, parse("terminal-margin-horizontal = 3").properties.terminalMarginHorizontalDp)
        assertEquals(3, parse("terminal-margin-horizontal = -10").properties.terminalMarginHorizontalDp)
        assertEquals(0, parse("terminal-margin-vertical = 999").properties.terminalMarginVerticalDp)
    }

    @Test
    fun `an out of range value is reported`() {
        val loaded = parse("terminal-margin-horizontal = 999")
        assertTrue(
            "越界必须留痕，否则用户不知道自己写的值被改过: ${loaded.errors}",
            loaded.errors.any { it.contains("terminal-margin-horizontal") },
        )
    }

    @Test
    fun `transcript rows outside the range fall back to the default`() {
        assertEquals(2000, parse("terminal-transcript-rows = 2000").properties.terminalTranscriptRows)
        assertEquals(2000, parse("terminal-transcript-rows = 10").properties.terminalTranscriptRows)
        assertEquals(2000, parse("terminal-transcript-rows = 99999").properties.terminalTranscriptRows)
    }

    @Test
    fun `a non numeric transcript rows keeps the default`() {
        val loaded = parse("terminal-transcript-rows = many")
        assertEquals(2000, loaded.properties.terminalTranscriptRows)
        assertTrue(
            "非法值必须留痕: ${loaded.errors}",
            loaded.errors.any { it.contains("terminal-transcript-rows") },
        )
    }

    @Test
    fun `toolbar height outside the bounds falls back to the default`() {
        assertEquals(1f, parse("terminal-toolbar-height = 1.0").properties.terminalToolbarHeightScale, 0.001f)
        assertEquals(1f, parse("terminal-toolbar-height = 0.1").properties.terminalToolbarHeightScale, 0.001f)
        assertEquals(1f, parse("terminal-toolbar-height = 9").properties.terminalToolbarHeightScale, 0.001f)
    }

    @Test
    fun `enumerated values map to their enums`() {
        assertEquals(
            TerminalCursorStyle.UNDERLINE,
            parse("terminal-cursor-style = underline").properties.terminalCursorStyle,
        )
        // `beam` 是竖线的别名（`config.md` 允许的取值），落成 `bar`。
        assertEquals(
            TerminalCursorStyle.BAR,
            parse("terminal-cursor-style = beam").properties.terminalCursorStyle,
        )
        assertEquals(NightMode.TRUE, parse("night-mode = true").properties.nightMode)
        assertEquals(NightMode.FALSE, parse("night-mode = false").properties.nightMode)
        assertEquals(ExtraKeysStyle.ALL, parse("extra-keys-style = all").properties.extraKeysStyle)
        assertEquals(BackKeyBehaviour.ESCAPE, parse("back-key = escape").properties.backKey)
    }

    /** 五种样式都必须被认得：写错一个就等于用户的键栏外观莫名其妙地回到默认。 */
    @Test
    fun `every extra keys style parses`() {
        assertEquals(ExtraKeysStyle.DEFAULT, parse("extra-keys-style = default").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.ARROWS_ONLY, parse("extra-keys-style = arrows-only").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.ARROWS_ALL, parse("extra-keys-style = arrows-all").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.ALL, parse("extra-keys-style = all").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.NONE, parse("extra-keys-style = none").properties.extraKeysStyle)
    }

    /** 大小写不该让一个合法的枚举值失效：`Underline` 与 `underline` 是同一个东西。 */
    @Test
    fun `enum values are matched case-insensitively`() {
        assertEquals(
            TerminalCursorStyle.BAR,
            parse("terminal-cursor-style = Bar").properties.terminalCursorStyle,
        )
    }

    @Test
    fun `an unknown enum value keeps the default and reports it`() {
        val loaded = parse("back-key = sideways")
        assertEquals(BackKeyBehaviour.BACK, loaded.properties.backKey)
        assertTrue(loaded.errors.any { it.contains("back-key") })
    }

    @Test
    fun `shortcuts are single characters and normalised to lower case`() {
        val loaded = parse("shortcut.next-session = N")
        assertEquals('n', loaded.properties.shortcutNextSession)
    }

    /** 多字符的快捷键值只禁用该快捷键，不影响其余解析。 */
    @Test
    fun `a multi-character shortcut is disabled and reported`() {
        val loaded = parse("shortcut.create-session = ctrl+c")
        assertNull(loaded.properties.shortcutCreateSession)
        assertTrue(loaded.errors.any { it.contains("shortcut.create-session") })
    }

    /** 未写 `shortcut.*` 时必须是 null 而不是某个默认字符：默认值由应用决定，不由解析器猜。 */
    @Test
    fun `absent shortcuts stay null`() {
        val loaded = parse("fullscreen = true")
        assertNull(loaded.properties.shortcutCreateSession)
        assertNull(loaded.properties.shortcutNextSession)
        assertNull(loaded.properties.shortcutPreviousSession)
    }

    @Test
    fun `default working directory is trimmed`() {
        assertEquals(
            "/sdcard/projects",
            parse("default-working-directory =   /sdcard/projects  ").properties.defaultWorkingDirectory,
        )
    }

    /** 注释与空行被忽略，`#` 与 `!` 都是注释起始符（Java properties 约定）。 */
    @Test
    fun `comments and blank lines are ignored`() {
        val loaded = parse(
            """
            # a comment
            ! another comment style

            fullscreen = true
            """.trimIndent(),
        )
        assertTrue(loaded.properties.fullscreen)
        assertTrue(loaded.errors.isEmpty())
    }

    /** 重复键后者生效，但必须留痕——用户写两行通常是在排查「改了没效果」。 */
    @Test
    fun `duplicate keys use the last value and report the duplicate`() {
        val loaded = parse(
            """
            fullscreen = false
            fullscreen = true
            """.trimIndent(),
        )
        assertTrue(loaded.properties.fullscreen)
        assertTrue(loaded.errors.any { it.contains("duplicate") })
    }

    @Test
    fun `extra-keys overrides the default layout`() {
        val loaded = parse("""extra-keys = [['A','B'],['C']]""")
        assertEquals(
            listOf(listOf(ExtraKey("A"), ExtraKey("B")), listOf(ExtraKey("C"))),
            loaded.properties.extraKeys,
        )
        assertTrue(loaded.errors.isEmpty())
    }

    /** 写坏的 `extra-keys` 退回默认布局，而不是让整份配置作废。 */
    @Test
    fun `a malformed extra-keys keeps the default layout`() {
        val loaded = parse("extra-keys = [[`")
        assertEquals(TermuxProperties.DEFAULT_EXTRA_KEYS, loaded.properties.extraKeys)
        assertTrue(loaded.errors.any { it.contains("extra-keys") })
    }

    /** `extra-keys` 的 popup 语法：`{key: '-', popup: '|'}`。 */
    @Test
    fun `extra-keys popup is parsed`() {
        val loaded = parse("""extra-keys = [['/',{key:'-',popup:'|'}]]""")
        assertEquals(
            listOf(listOf(ExtraKey("/"), ExtraKey("-", popup = "|"))),
            loaded.properties.extraKeys,
        )
    }

    /**
     * 宏是 termux 的正式语法（`{macro: "CTRL f d"}`），本应用只解析、不播放。
     * 它 MUST 只丢自己那一项：整份布局因为一个宏回到默认，等于用户配好的键位被
     * 一个不认识的对象整体作废。
     */
    @Test
    fun `a macro key does not void the whole layout`() {
        val loaded = parse("""extra-keys = [['ESC',{macro:'CTRL f d',display:'tmux'}]]""")
        assertEquals(
            listOf(listOf(ExtraKey("ESC"), ExtraKey("tmux", macro = "CTRL f d"))),
            loaded.properties.extraKeys,
        )
        assertTrue("宏本身不是错误: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /** 对象形式的 popup（宏）被跳过并留证，主键本身照常保留。 */
    @Test
    fun `an object popup is dropped without losing the key`() {
        val loaded = parse("""extra-keys = [[{key:'TAB',popup:{macro:'SHIFT TAB'}}]]""")
        assertEquals(listOf(listOf(ExtraKey("TAB"))), loaded.properties.extraKeys)
        assertTrue(
            "丢掉 popup 必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("popup") },
        )
    }
}
