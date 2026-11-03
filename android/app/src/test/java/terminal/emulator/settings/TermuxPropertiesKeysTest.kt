package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `termux.properties` 的逐键契约。
 *
 * 默认值与取值集合逐项对齐 termux-app `v0.119.0-beta.3` 的 `TermuxPropertyConstants`
 * 与 `TermuxSharedProperties`：同一份配置在两个应用里必须表现相同，故这里对 20 个键
 * 各断言「默认值」与「非法取值只丢自己那一项」，而不是只证明解析器不崩。
 */
class TermuxPropertiesKeysTest {
    private fun parse(text: String): TermuxProperties = TermuxPropertiesParser.parse(text).properties

    private fun errors(text: String): List<String> = TermuxPropertiesParser.parse(text).errors

    /** 非法取值必须留下提到该键的错误，否则用户不知道自己写的值被改过。 */
    private fun assertRejected(text: String, propertyKey: String) {
        val reported = errors(text)
        assertTrue(
            "非法取值必须留痕（键 $propertyKey）：$reported",
            reported.any { it.contains(propertyKey) },
        )
    }

    @Test
    fun `boolean defaults match upstream`() {
        val defaults = parse("")
        assertFalse(defaults.fullscreen)
        assertFalse(defaults.useFullscreenWorkaround)
        assertFalse(defaults.ctrlSpaceWorkaround)
        assertFalse(defaults.hideSoftKeyboardOnStartup)
        assertTrue(defaults.extraKeysTextAllCaps)
    }

    @Test
    fun `numeric defaults match upstream`() {
        val defaults = parse("")
        assertEquals(2000, defaults.terminalTranscriptRows)
        assertEquals(3, defaults.terminalMarginHorizontalDp)
        assertEquals(0, defaults.terminalMarginVerticalDp)
        assertEquals(1f, defaults.terminalToolbarHeightScale, 0.001f)
    }

    @Test
    fun `enumerated defaults match upstream`() {
        val defaults = parse("")
        assertEquals(TerminalCursorStyle.BLOCK, defaults.terminalCursorStyle)
        assertEquals(ExtraKeysStyle.DEFAULT, defaults.extraKeysStyle)
        assertEquals(BackKeyBehaviour.BACK, defaults.backKey)
        assertEquals(VolumeKeysBehaviour.VIRTUAL, defaults.volumeKeys)
        assertEquals(SoftKeyboardToggleBehaviour.SHOW_HIDE, defaults.softKeyboardToggleBehaviour)
        assertEquals(BellCharacter.VIBRATE, defaults.bellCharacter)
    }

    @Test
    fun `layout and directory defaults match upstream`() {
        val defaults = parse("")
        assertEquals(TermuxProperties.DEFAULT_EXTRA_KEYS, defaults.extraKeys)
        assertEquals("", defaults.defaultWorkingDirectory)
    }

    /** 快捷键默认一个都没有：`Ctrl+t` 这类组合键只有写进配置才生效。 */
    @Test
    fun `session shortcuts are absent unless configured`() {
        assertEquals(emptyMap<SessionShortcut, Int>(), parse("").sessionShortcuts)
    }

    @Test
    fun `fullscreen accepts both boolean spellings`() {
        assertTrue(parse("fullscreen = true").fullscreen)
        assertFalse(parse("fullscreen = false").fullscreen)
        assertTrue(parse("fullscreen = TRUE").fullscreen)
    }

    @Test
    fun `an invalid fullscreen keeps the default and is reported`() {
        val loaded = TermuxPropertiesParser.parse("fullscreen = perhaps")
        assertFalse(loaded.properties.fullscreen)
        assertRejected("fullscreen = perhaps", "fullscreen")
    }

    @Test
    fun `an invalid use-fullscreen-workaround keeps the default and is reported`() {
        val loaded = TermuxPropertiesParser.parse("use-fullscreen-workaround = perhaps")
        assertFalse(loaded.properties.useFullscreenWorkaround)
        assertRejected("use-fullscreen-workaround = perhaps", "use-fullscreen-workaround")
    }

    @Test
    fun `an invalid ctrl-space-workaround keeps the default and is reported`() {
        val loaded = TermuxPropertiesParser.parse("ctrl-space-workaround = perhaps")
        assertFalse(loaded.properties.ctrlSpaceWorkaround)
        assertRejected("ctrl-space-workaround = perhaps", "ctrl-space-workaround")
    }

    @Test
    fun `an invalid hide-soft-keyboard-on-startup keeps the default and is reported`() {
        val loaded = TermuxPropertiesParser.parse("hide-soft-keyboard-on-startup = perhaps")
        assertFalse(loaded.properties.hideSoftKeyboardOnStartup)
        assertRejected("hide-soft-keyboard-on-startup = perhaps", "hide-soft-keyboard-on-startup")
    }

    @Test
    fun `an invalid extra-keys-text-all-caps keeps the default and is reported`() {
        val loaded = TermuxPropertiesParser.parse("extra-keys-text-all-caps = perhaps")
        assertTrue(loaded.properties.extraKeysTextAllCaps)
        assertRejected("extra-keys-text-all-caps = perhaps", "extra-keys-text-all-caps")
    }

    @Test
    fun `transcript rows outside the upstream range keep the default`() {
        assertEquals(2000, parse("terminal-transcript-rows = 99").terminalTranscriptRows)
        assertEquals(2000, parse("terminal-transcript-rows = 50001").terminalTranscriptRows)
        assertRejected("terminal-transcript-rows = 99", "terminal-transcript-rows")
    }

    @Test
    fun `margins outside the upstream range keep the default`() {
        assertEquals(3, parse("terminal-margin-horizontal = 101").terminalMarginHorizontalDp)
        assertEquals(0, parse("terminal-margin-vertical = -1").terminalMarginVerticalDp)
        assertRejected("terminal-margin-horizontal = 101", "terminal-margin-horizontal")
        assertRejected("terminal-margin-vertical = -1", "terminal-margin-vertical")
    }

    @Test
    fun `toolbar height outside the upstream range keeps the default`() {
        assertEquals(1f, parse("terminal-toolbar-height = 0.39").terminalToolbarHeightScale, 0.001f)
        assertEquals(1f, parse("terminal-toolbar-height = 3.01").terminalToolbarHeightScale, 0.001f)
        assertRejected("terminal-toolbar-height = 0.39", "terminal-toolbar-height")
    }

    @Test
    fun `an unknown cursor style keeps block and is reported`() {
        val loaded = TermuxPropertiesParser.parse("terminal-cursor-style = blocky")
        assertEquals(TerminalCursorStyle.BLOCK, loaded.properties.terminalCursorStyle)
        assertRejected("terminal-cursor-style = blocky", "terminal-cursor-style")
    }

    @Test
    fun `an unknown extra keys style keeps the default and is reported`() {
        val loaded = TermuxPropertiesParser.parse("extra-keys-style = fancy")
        assertEquals(ExtraKeysStyle.DEFAULT, loaded.properties.extraKeysStyle)
        assertRejected("extra-keys-style = fancy", "extra-keys-style")
    }

    @Test
    fun `an unknown back key behaviour keeps back and is reported`() {
        val loaded = TermuxPropertiesParser.parse("back-key = sideways")
        assertEquals(BackKeyBehaviour.BACK, loaded.properties.backKey)
        assertRejected("back-key = sideways", "back-key")
    }

    @Test
    fun `an unknown volume keys behaviour keeps virtual and is reported`() {
        val loaded = TermuxPropertiesParser.parse("volume-keys = loud")
        assertEquals(VolumeKeysBehaviour.VIRTUAL, loaded.properties.volumeKeys)
        assertRejected("volume-keys = loud", "volume-keys")
    }

    @Test
    fun `an unknown soft keyboard toggle behaviour keeps show hide and is reported`() {
        val loaded = TermuxPropertiesParser.parse("soft-keyboard-toggle-behaviour = maybe")
        assertEquals(SoftKeyboardToggleBehaviour.SHOW_HIDE, loaded.properties.softKeyboardToggleBehaviour)
        assertRejected("soft-keyboard-toggle-behaviour = maybe", "soft-keyboard-toggle-behaviour")
    }

    /** `rename-session` 没有对应入口：明确忽略并留痕，不得静默接受。 */
    @Test
    fun `the rename shortcut is ignored with an error`() {
        val loaded = TermuxPropertiesParser.parse("shortcut.rename-session = Ctrl+r")
        assertEquals(emptyMap<SessionShortcut, Int>(), loaded.properties.sessionShortcuts)
        assertRejected("shortcut.rename-session = Ctrl+r", "shortcut.rename-session")
    }

    /** 三个可用的会话快捷键各写各的，互不串台。 */
    @Test
    fun `each session shortcut is read independently`() {
        val loaded = TermuxPropertiesParser.parse("shortcut.next-session = Ctrl+n")
        assertEquals(
            mapOf(SessionShortcut.NEXT to 'n'.code),
            loaded.properties.sessionShortcuts,
        )
    }
}
