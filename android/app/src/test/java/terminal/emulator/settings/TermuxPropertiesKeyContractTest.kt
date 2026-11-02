package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `termux.properties` 的**取值集合**契约。
 *
 * [TermuxPropertiesKeysTest] 钉住默认值与非法取值；这里钉住合法取值必须生效，
 * 并逐键对齐 termux-app `v0.119.0-beta.3` 的 `TermuxPropertyConstants`：
 * 上游没有的别名（如 `beam`）不接受，上游有的写法不能被漏掉。
 */
class TermuxPropertiesKeyContractTest {
    private fun parse(text: String) = TermuxPropertiesParser.parse(text)

    @Test
    fun `terminal cursor style accepts the upstream set`() {
        assertEquals(TerminalCursorStyle.BLOCK, parse("terminal-cursor-style = block").properties.terminalCursorStyle)
        assertEquals(TerminalCursorStyle.BAR, parse("terminal-cursor-style = bar").properties.terminalCursorStyle)
        assertEquals(
            TerminalCursorStyle.UNDERLINE,
            parse("terminal-cursor-style = underline").properties.terminalCursorStyle,
        )
    }

    /** 上游没有 `beam`：接受它等于凭空扩了取值集合，别名不算兼容。 */
    @Test
    fun `terminal cursor style rejects the non-upstream beam alias`() {
        val loaded = parse("terminal-cursor-style = beam")
        assertEquals(TerminalCursorStyle.BLOCK, loaded.properties.terminalCursorStyle)
        assertTrue(
            "非法光标样式必须留痕：${loaded.errors}",
            loaded.errors.any { it.contains("terminal-cursor-style") },
        )
    }

    @Test
    fun `extra keys style accepts every upstream value`() {
        assertEquals(ExtraKeysStyle.DEFAULT, parse("extra-keys-style = default").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.ARROWS_ONLY, parse("extra-keys-style = arrows-only").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.ARROWS_ALL, parse("extra-keys-style = arrows-all").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.ALL, parse("extra-keys-style = all").properties.extraKeysStyle)
        assertEquals(ExtraKeysStyle.NONE, parse("extra-keys-style = none").properties.extraKeysStyle)
    }

    @Test
    fun `back key accepts back and escape`() {
        assertEquals(BackKeyBehaviour.BACK, parse("back-key = back").properties.backKey)
        assertEquals(BackKeyBehaviour.ESCAPE, parse("back-key = escape").properties.backKey)
    }

    @Test
    fun `volume keys accepts virtual and volume`() {
        assertEquals(VolumeKeysBehaviour.VIRTUAL, parse("volume-keys = virtual").properties.volumeKeys)
        assertEquals(VolumeKeysBehaviour.VOLUME, parse("volume-keys = volume").properties.volumeKeys)
    }

    @Test
    fun `soft keyboard toggle accepts both upstream values`() {
        assertEquals(
            SoftKeyboardToggleBehaviour.SHOW_HIDE,
            parse("soft-keyboard-toggle-behaviour = show/hide").properties.softKeyboardToggleBehaviour,
        )
        assertEquals(
            SoftKeyboardToggleBehaviour.ENABLE_DISABLE,
            parse("soft-keyboard-toggle-behaviour = enable/disable").properties.softKeyboardToggleBehaviour,
        )
    }

    @Test
    fun `boolean keys accept true and false`() {
        assertTrue(parse("fullscreen = true").properties.fullscreen)
        assertFalse(parse("fullscreen = false").properties.fullscreen)
        assertTrue(parse("use-fullscreen-workaround = true").properties.useFullscreenWorkaround)
        assertFalse(parse("use-fullscreen-workaround = false").properties.useFullscreenWorkaround)
        assertTrue(parse("ctrl-space-workaround = true").properties.ctrlSpaceWorkaround)
        assertFalse(parse("ctrl-space-workaround = false").properties.ctrlSpaceWorkaround)
        assertTrue(parse("hide-soft-keyboard-on-startup = true").properties.hideSoftKeyboardOnStartup)
        assertFalse(parse("hide-soft-keyboard-on-startup = false").properties.hideSoftKeyboardOnStartup)
        assertTrue(parse("extra-keys-text-all-caps = true").properties.extraKeysTextAllCaps)
        assertFalse(parse("extra-keys-text-all-caps = false").properties.extraKeysTextAllCaps)
    }

    @Test
    fun `numeric keys accept values inside the upstream range`() {
        // 上游范围是 100..50000（`TerminalEmulator.TERMINAL_TRANSCRIPT_ROWS_MIN/MAX`），
        // 边界值必须生效。
        assertEquals(2000, parse("terminal-transcript-rows = 2000").properties.terminalTranscriptRows)
        assertEquals(100, parse("terminal-transcript-rows = 100").properties.terminalTranscriptRows)
        assertEquals(50000, parse("terminal-transcript-rows = 50000").properties.terminalTranscriptRows)
        assertEquals(0, parse("terminal-margin-horizontal = 0").properties.terminalMarginHorizontalDp)
        assertEquals(100, parse("terminal-margin-horizontal = 100").properties.terminalMarginHorizontalDp)
        assertEquals(0, parse("terminal-margin-vertical = 0").properties.terminalMarginVerticalDp)
        assertEquals(100, parse("terminal-margin-vertical = 100").properties.terminalMarginVerticalDp)
        assertEquals(0.4f, parse("terminal-toolbar-height = 0.4").properties.terminalToolbarHeightScale, 0.001f)
        assertEquals(3f, parse("terminal-toolbar-height = 3.0").properties.terminalToolbarHeightScale, 0.001f)
    }

    @Test
    fun `default working directory is taken verbatim`() {
        assertEquals(
            "/data/data/com.termux/files/home",
            parse("default-working-directory = /data/data/com.termux/files/home").properties.defaultWorkingDirectory,
        )
        assertEquals("", parse("default-working-directory =").properties.defaultWorkingDirectory)
    }

    @Test
    fun `session shortcuts accept ctrl and alt combinations`() {
        val loaded = parse(
            """
            shortcut.create-session = Ctrl+t
            shortcut.next-session = Ctrl+n
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
        assertTrue("合法快捷键不得报错：${loaded.errors}", loaded.errors.isEmpty())
    }

    @Test
    fun `extra keys layout replaces the default layout`() {
        val loaded = parse("extra-keys = [['ESC','CTRL']]")
        assertEquals(listOf(listOf(ExtraKey("ESC"), ExtraKey("CTRL"))), loaded.properties.extraKeys)
        assertTrue("合法键栏不得报错：${loaded.errors}", loaded.errors.isEmpty())
    }

    /**
     * 上游的 `shortcut.rename-session` 不在声明键集里：不生效，但要明确报告。
     *
     * 用户从 termux 搬配置时多半带着这个键，静默丢掉会让人以为重命名快捷键在生效。
     */
    @Test
    fun `the upstream rename shortcut is skipped and reported`() {
        val loaded = parse("shortcut.rename-session = Ctrl+r")
        assertTrue(loaded.properties.sessionShortcuts.isEmpty())
        assertTrue(
            "不支持的键必须留痕：${loaded.errors}",
            loaded.errors.any { it.contains("shortcut.rename-session") },
        )
    }

    /** 未声明的键直接跳过：不报错，也不影响任何其他键。 */
    @Test
    fun `unknown keys are skipped without touching the rest`() {
        val loaded = parse("some-unknown-key = true\nfullscreen = true")
        assertTrue(loaded.properties.fullscreen)
        assertTrue("未知键不得报错：${loaded.errors}", loaded.errors.isEmpty())
    }
}
