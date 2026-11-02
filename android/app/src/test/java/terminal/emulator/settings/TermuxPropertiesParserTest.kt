package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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

    /**
     * 默认启动即弹键盘：与上游 termux 同值，也是 `DESIGN.md` 的要求。
     *
     * 该键在还是空转键时取过 `true`（那时它只被解析、不改变行为），键变成真开关后
     * 默认值没有跟着上游一起复核，于是默认配置下冷启动永远不弹输入法。
     */
    @Test
    fun `keyboard startup default matches upstream`() {
        assertFalse(parse("").properties.hideSoftKeyboardOnStartup)
    }

    @Test
    fun `booleans are read case-insensitively`() {
        val loaded = parse(
            """
            fullscreen = TRUE
            ctrl-space-workaround=true
            """.trimIndent(),
        )
        assertTrue(loaded.properties.fullscreen)
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
        assertEquals(TerminalCursorStyle.BAR, parse("terminal-cursor-style = bar").properties.terminalCursorStyle)
        assertEquals(ExtraKeysStyle.ALL, parse("extra-keys-style = all").properties.extraKeysStyle)
        assertEquals(BackKeyBehaviour.ESCAPE, parse("back-key = escape").properties.backKey)
    }

    /** 上游没有 `beam`：别名不是配置契约，非法值回退默认值并留痕。 */
    @Test
    fun `the non-upstream beam alias falls back to the default`() {
        val loaded = parse("terminal-cursor-style = beam")
        assertEquals(TerminalCursorStyle.BLOCK, loaded.properties.terminalCursorStyle)
        assertTrue(
            "非法光标样式必须留痕：${loaded.errors}",
            loaded.errors.any { it.contains("terminal-cursor-style") },
        )
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

    /**
     * `extra-keys` 的 popup 语法：`{key: '-', popup: '|'}`。
     *
     * popup 是一个完整的按键（上游 termux 的 `ExtraKeysInfo.normalizeKeyConfig` 把
     * 字符串包成 `{key: 'X'}`），故解析结果是嵌套的按键，不是裸字符串。
     */
    @Test
    fun `extra-keys popup is parsed`() {
        val loaded = parse("""extra-keys = [['/',{key:'-',popup:'|'}]]""")
        assertEquals(
            listOf(listOf(ExtraKey("/"), ExtraKey("-", popup = ExtraKey("|")))),
            loaded.properties.extraKeys,
        )
    }

    /**
     * 宏是 termux 的正式语法（`{macro: "CTRL f d"}`），本应用解析并播放。
     * 它 MUST 只丢自己那一项：整份布局因为一个宏回到默认，等于用户配好的键位被
     * 一个不认识的对象整体作废。
     */
    @Test
    fun `a macro key does not void the whole layout`() {
        val loaded = parse("""extra-keys = [['ESC',{macro:'CTRL f d',display:'tmux'}]]""")
        assertEquals(
            listOf(listOf(ExtraKey("ESC"), ExtraKey("tmux", macro = "CTRL f d", display = "tmux"))),
            loaded.properties.extraKeys,
        )
        assertTrue("宏本身不是错误: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /**
     * 对象形式的 popup 是上游的正式语法（`{key: 'X', popup: {macro: '...'}}`），
     * 解析出次级按键，主键本身照常保留。
     */
    @Test
    fun `an object popup is parsed as a nested key`() {
        val loaded = parse("""extra-keys = [[{key:'TAB',popup:{macro:'SHIFT TAB'}}]]""")
        assertEquals(
            listOf(listOf(ExtraKey("TAB", popup = ExtraKey(name = "", macro = "SHIFT TAB")))),
            loaded.properties.extraKeys,
        )
        assertTrue("对象形式的 popup 不是错误: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /** `shortcut.*` 解析成「小写码点 → 动作」，Ctrl+Shift+N 也能命中。 */
    @Test
    fun `session shortcuts parse ctrl combinations`() {
        val loaded =
            parse(
                """
                shortcut.create-session = Ctrl+t
                shortcut.next-session = Ctrl+N
                shortcut.previous-session = Ctrl+p
                """.trimIndent(),
            )
        assertEquals(
            mapOf(
                SessionShortcut.CREATE to 't'.code,
                SessionShortcut.NEXT to 'n'.code,
                SessionShortcut.PREVIOUS to 'p'.code,
            ),
            loaded.properties.sessionShortcuts,
        )
        assertTrue("合法快捷键不是错误: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /** 不是 `Ctrl+<字符>` 的值不生效，但要留痕。 */
    @Test
    fun `an invalid session shortcut is ignored with an error`() {
        val loaded = parse("shortcut.next-session = Alt+n")
        assertEquals(emptyMap<SessionShortcut, Int>(), loaded.properties.sessionShortcuts)
        assertTrue(loaded.errors.any { it.contains("shortcut.next-session") })
    }

    /** 没有重命名 UI 时，`rename-session` 明确忽略而不是假装支持。 */
    @Test
    fun `rename shortcut is reported as unsupported`() {
        val loaded = parse("shortcut.rename-session = Ctrl+r")
        assertEquals(emptyMap<SessionShortcut, Int>(), loaded.properties.sessionShortcuts)
        assertTrue(loaded.errors.any { it.contains("rename") })
    }

    /** 空键名没有可发送的内容，等同于没有 key。 */
    @Test
    fun `an empty key name drops only itself`() {
        val loaded = parse("""extra-keys = [['ESC',{key:''}]]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"))), loaded.properties.extraKeys)
        assertTrue(loaded.errors.any { it.contains("dropped") })
    }

    /** 对象字段名既可裸写也可带引号（上游 JSON 两者都收）。 */
    @Test
    fun `quoted object field names are accepted`() {
        val loaded = parse("""extra-keys = [[{"key":"ESC","display":"quit"}]]""")
        assertEquals(listOf(listOf(ExtraKey("ESC", display = "quit"))), loaded.properties.extraKeys)
        assertTrue("带引号的字段名不是错误: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /** popup 对象里没有 `key` 也没有 `macro`：只丢这个 popup，主键照常保留。 */
    @Test
    fun `an unusable object popup is dropped without losing the key`() {
        val loaded = parse("""extra-keys = [[{key:'TAB',popup:{display:'x'}}]]""")
        assertEquals(listOf(listOf(ExtraKey("TAB"))), loaded.properties.extraKeys)
        assertTrue(
            "丢掉 popup 必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("popup") },
        )
    }

    /**
     * popup 对象里的语法错误只丢这个 popup，主键照常保留。
     *
     * popup 是次级键（长按才触发），主键才是日常使用的那一个。popup 里少一个逗号
     * 就把主键一起丢掉，等于用户为一个长按动作赔上整个键位；整行因此被掏空时更会
     * 让**整份布局**退回默认——与「写坏的项只丢自己那一项」的约定直接冲突。
     */
    @Test
    fun `a malformed popup object drops only the popup`() {
        val loaded = parse("""extra-keys = [[{key:'TAB',popup:{key:'X' bad}},'CTRL']]""")
        assertEquals(
            listOf(listOf(ExtraKey("TAB"), ExtraKey("CTRL"))),
            loaded.properties.extraKeys,
        )
        assertTrue(
            "丢掉坏 popup 必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("popup") },
        )
    }

    /** 同时写 `key` 与 `macro` 是上游明确禁止的：只丢这一项，其余键位照常。 */
    @Test
    fun `a key with both key and macro drops only itself`() {
        val loaded = parse("""extra-keys = [['ESC',{key:'TAB',macro:'CTRL f'}]]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"))), loaded.properties.extraKeys)
        assertTrue(
            "同时写 key 与 macro 必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("macro") },
        )
    }

    /**
     * 既没有 `key` 也没有 `macro` 的对象只丢自己那一项。
     *
     * 整份布局因为一个空对象回到默认，等于用户配好的其余键位被一个无内容的对象
     * 整体作废——这正是「只丢一项」约定要防的情形。
     */
    @Test
    fun `an empty object drops only itself`() {
        val loaded = parse("""extra-keys = [['ESC',{popup:'|'},'TAB']]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"), ExtraKey("TAB"))), loaded.properties.extraKeys)
        assertTrue(
            "丢掉坏项必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("dropped") },
        )
    }

    /**
     * 对象里的尾随逗号（`{key: 'X',}`）照常解析出这个键。
     *
     * 上游用的 JSON 解析器同样收这种写法。丢掉整个键位等于让用户为一个逗号白配一次。
     */
    @Test
    fun `a key object with a trailing comma keeps the key`() {
        val loaded = parse("""extra-keys = [['ESC',{key:'TAB',},'CTRL']]""")
        assertEquals(
            listOf(listOf(ExtraKey("ESC"), ExtraKey("TAB"), ExtraKey("CTRL"))),
            loaded.properties.extraKeys,
        )
        assertTrue(
            "尾随逗号必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("trailing comma") },
        )
    }

    /**
     * 语法坏掉的对象（缺冒号）只丢自己那一项。
     *
     * 丢整份布局等于让用户配好的其余键位因为一个冒号全部消失——与「只丢一项」的
     * 约定直接冲突。
     */
    @Test
    fun `a key object missing its colon drops only itself`() {
        val loaded = parse("""extra-keys = [['ESC',{key 'TAB'},'CTRL']]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"), ExtraKey("CTRL"))), loaded.properties.extraKeys)
        assertTrue(
            "丢掉坏项必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("dropped") },
        )
    }

    /** 空对象（`{}`）没有可执行内容：只丢自己那一项，其余键位照常。 */
    @Test
    fun `an empty key object drops only itself`() {
        val loaded = parse("""extra-keys = [['ESC',{},'CTRL']]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"), ExtraKey("CTRL"))), loaded.properties.extraKeys)
        assertTrue(
            "丢掉坏项必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("dropped") },
        )
    }

    /** 整行都是坏项时整行消失，其余行照常保留。 */
    @Test
    fun `a row of dropped objects disappears without voiding the layout`() {
        val loaded = parse("""extra-keys = [[{popup:'|'}],['ESC']]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"))), loaded.properties.extraKeys)
    }

    /** `display` 与该键同时存在时保留下来，供键帽覆盖字形。 */
    @Test
    fun `a display label is kept for a normal key`() {
        val loaded = parse("""extra-keys = [[{key:'ESC',display:'quit'}]]""")
        assertEquals(listOf(listOf(ExtraKey("ESC", display = "quit"))), loaded.properties.extraKeys)
    }

    /** 只有 `display` 的对象没有可执行内容，只丢自己那一项。 */
    @Test
    fun `a display-only object drops only itself`() {
        val loaded = parse("""extra-keys = [['ESC',{display:'quit'}]]""")
        assertEquals(listOf(listOf(ExtraKey("ESC"))), loaded.properties.extraKeys)
        assertTrue(
            "丢掉 display-only 项必须留下可读的说明: ${loaded.errors}",
            loaded.errors.any { it.contains("dropped") },
        )
    }

    /** 显式空布局是「不要键栏」，不是写坏：MUST 保留零行。 */
    @Test
    fun `an explicitly empty layout stays empty`() {
        val loaded = parse("extra-keys = []")
        assertEquals(emptyList<List<ExtraKey>>(), loaded.properties.extraKeys)
        assertTrue("显式清空不是错误: ${loaded.errors}", loaded.errors.isEmpty())
    }

    /** 一个键都不剩的布局视为写坏：键栏是抽屉与滚动锁定的唯一入口。 */
    @Test
    fun `a layout with no usable keys falls back to the default`() {
        val loaded = parse("""extra-keys = [[{popup:'|'}]]""")
        assertEquals(TermuxProperties.DEFAULT_EXTRA_KEYS, loaded.properties.extraKeys)
        assertTrue(loaded.errors.any { it.contains("extra-keys") })
    }
}
