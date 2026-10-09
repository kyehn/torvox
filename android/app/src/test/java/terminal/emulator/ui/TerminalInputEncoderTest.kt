package terminal.emulator.ui

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TerminalInputEncoderTest {
    private fun enc(text: String, ctrl: Boolean = false, alt: Boolean = false): ByteArray =
        TerminalInputEncoder.encodeCommittedText(text, ctrl, alt)

    private fun bytes(vararg values: Int): ByteArray = ByteArray(values.size) { values[it].toByte() }

    @Test
    fun `plain text is utf8 encoded`() {
        assertArrayEquals(bytes(0x61, 0x62, 0x63), enc("abc"))
    }

    @Test
    fun `ctrl folds single char to control byte`() {
        assertArrayEquals(bytes(0x01), enc("a", ctrl = true))
        assertArrayEquals(bytes(0x03), enc("c", ctrl = true))
    }

    @Test
    fun `ctrl with multi char commit does not fold`() {
        // IME pinyin commit "abc" must NOT become 0x01 0x02 0x03
        assertArrayEquals(bytes(0x61, 0x62, 0x63), enc("abc", ctrl = true))
    }

    @Test
    fun `ctrl 1 9 0 emit csi 27 instead of being dropped`() {
        // zed-port mappings/keys.rs: digits with no traditional caret fold
        // are sent as `CSI 27;5;code~` (see docs/specification/REFERENCE.md).
        assertArrayEquals(csi27(5, '1'.code), enc("1", ctrl = true))
        assertArrayEquals(csi27(5, '9'.code), enc("9", ctrl = true))
        assertArrayEquals(csi27(5, '0'.code), enc("0", ctrl = true))
    }

    @Test
    fun `ctrl alt digit prefixes esc to csi 27`() {
        // Ctrl+Alt+1 → ESC [ 27 ; 7 ; 49 ~ (xterm/zed modifier: 1 base +
        // alt 2 + ctrl 4 — same scheme as CSI 1;mod A arrow sequences).
        assertArrayEquals(csi27(7, '1'.code), enc("1", ctrl = true, alt = true))
    }

    private fun csi27(modifier: Int, code: Int): ByteArray = "\u001b[27;$modifier;$code~".toByteArray(Charsets.UTF_8)

    @Test
    fun `alt prefixes esc`() {
        assertArrayEquals(bytes(0x1B, 0x61), enc("a", alt = true))
        assertArrayEquals(bytes(0x1B, 0x01), enc("a", ctrl = true, alt = true))
    }

    @Test
    fun `ime newline commit becomes carriage return`() {
        // 多行字段的输入法回车走 commitText("\n")；终端回车是 CR，
        // 直接送 LF 会让 raw 模式的 TUI 应用（helix）把它读成 Ctrl+J 即字母 j。
        assertArrayEquals(bytes(0x0D), enc("\n"))
        assertArrayEquals(bytes(0x0D), enc("\n", ctrl = true))
        assertArrayEquals(bytes(0x0D), enc("\n", alt = true))
    }

    @Test
    fun `multi char commit keeps its own newlines`() {
        // 多字符提交（多行粘贴、输入法内部粘贴）里的换行是内容本身，
        // 不得被归一为 CR，否则多行文本会粘成一行。
        assertArrayEquals("a\nb".toByteArray(Charsets.UTF_8), enc("a\nb"))
    }

    @Test
    fun `emoji surrogate pair survives`() {
        assertArrayEquals("😀".toByteArray(Charsets.UTF_8), enc("😀"))
    }

    @Test
    fun `encodeKeyEvent enter produces carriage return`() {
        // 回车载荷是 CR(0x0D)。断言的是 VT 输入约定（termux KeyHandler、
        // ghostty function_keys.zig、kitty key_encoding.c、wezterm termwiz 一致），
        // 不是本实现的内部表示：写成 LF 会让 raw 模式下的 TUI 应用
        // （helix/vim/less）把回车读成 Ctrl+J 即字母 j。
        assertArrayEquals(
            bytes(0x0D),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_ENTER, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent arrow up produces csi`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x41),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_DPAD_UP, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent arrow right normal mode uses csi c`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x43),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_DPAD_RIGHT, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent numpad enter produces carriage return`() {
        // 数字小键盘回车与主回车同义（ghostty function_keys.zig 的
        // numpad_enter 非应用模式项即 "\r"）。
        assertArrayEquals(
            bytes(0x0D),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_NUMPAD_ENTER, 0, false, false),
        )
    }

    @Test
    fun `alt prefixes esc to every multi char commit`() {
        // Alt+drag/IME multi-char commit: xterm Meta prefixes each character.
        assertArrayEquals(bytes(0x1B, 0x61, 0x1B, 0x62), enc("ab", alt = true))
    }

    @Test
    fun `encodeKeyEvent ctrl c folds`() {
        assertArrayEquals(
            bytes(0x03),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_C, 'c'.code, true, false),
        )
    }

    @Test
    fun `encodeKeyEvent ctrl digit emits csi 27`() {
        // Hardware-key path mirrors the IME path (see docs/specification/REFERENCE.md).
        assertArrayEquals(
            "\u001b[27;5;49~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_1, '1'.code, true, false),
        )
        assertArrayEquals(
            "\u001b[27;5;57~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_9, '9'.code, true, false),
        )
        assertArrayEquals(
            "\u001b[27;5;48~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_0, '0'.code, true, false),
        )
    }

    @Test
    fun `encodeKeyEvent arrow uses csi in normal cursor mode`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x41),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                0,
                false,
                false,
                appCursorMode = false,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent arrow uses ss3 in application cursor mode`() {
        // DECCKM (see docs/specification/REFERENCE.md): app mode must emit ESC O A.
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x41),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                0,
                false,
                false,
                appCursorMode = true,
            ),
        )
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x42),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_DOWN,
                0,
                false,
                false,
                appCursorMode = true,
            ),
        )
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x43),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_RIGHT,
                0,
                false,
                false,
                appCursorMode = true,
            ),
        )
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x44),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_LEFT,
                0,
                false,
                false,
                appCursorMode = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent arrow with modifier stays csi even in app mode`() {
        // Modifier-carrying arrows use `CSI 1;mod A` regardless of DECCKM.
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x31, 0x3B, 0x35, 0x41),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                0,
                true,
                false,
                appCursorMode = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent unknown key with no char returns null`() {
        assertNull(TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_UNKNOWN, 0, false, false))
    }

    @Test
    fun `encodeKeyEvent tab produces tab byte`() {
        assertArrayEquals(
            bytes(0x09),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_TAB, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent alt tab produces csi 9 mod`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x39, 0x3B, 0x33, 0x7E), // ESC [ 9 ; 3 ~
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_TAB, 0, false, true),
        )
    }

    @Test
    fun `encodeKeyEvent ctrl tab produces csi 9 mod`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x39, 0x3B, 0x35, 0x7E), // ESC [ 9 ; 5 ~
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_TAB, 0, true, false),
        )
    }

    @Test
    fun `encodeKeyEvent alt enter produces esc carriage return`() {
        // ghostty function_keys.zig：Alt+回车在 modifyKeysNormal 下为 ESC + CR；
        // termux KeyHandler 同款返回值 "\033\r"。
        assertArrayEquals(
            bytes(0x1B, 0x0D),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_ENTER, 0, false, true),
        )
    }

    @Test
    fun `encodeKeyEvent backspace produces del byte`() {
        assertArrayEquals(
            bytes(0x7F),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_DEL, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent escape produces esc byte`() {
        assertArrayEquals(
            bytes(0x1B),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_ESCAPE, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent home produces csi h`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x48),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_MOVE_HOME, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent end produces csi f`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x46),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_MOVE_END, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent page up produces csi 5 tilde`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x35, 0x7E),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_PAGE_UP, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent f1 produces ss3 p`() {
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x50),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_F1, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent f2 f3 f4 use ss3 without modifiers`() {
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x51),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_F2, 0, false, false),
        )
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x52),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_F3, 0, false, false),
        )
        assertArrayEquals(
            bytes(0x1B, 0x4F, 0x53),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_F4, 0, false, false),
        )
    }

    @Test
    fun `encodeKeyEvent f5 to f12 use csi tilde without modifiers`() {
        val expected = mapOf(
            android.view.KeyEvent.KEYCODE_F5 to "\u001b[15~",
            android.view.KeyEvent.KEYCODE_F6 to "\u001b[17~",
            android.view.KeyEvent.KEYCODE_F7 to "\u001b[18~",
            android.view.KeyEvent.KEYCODE_F8 to "\u001b[19~",
            android.view.KeyEvent.KEYCODE_F9 to "\u001b[20~",
            android.view.KeyEvent.KEYCODE_F10 to "\u001b[21~",
            android.view.KeyEvent.KEYCODE_F11 to "\u001b[23~",
            android.view.KeyEvent.KEYCODE_F12 to "\u001b[24~",
        )
        expected.forEach { (keyCode, sequence) ->
            assertArrayEquals(
                sequence.toByteArray(Charsets.UTF_8),
                TerminalInputEncoder.encodeKeyEvent(keyCode, 0, false, false),
            )
        }
    }

    @Test
    fun `encodeKeyEvent f keys with modifiers use csi mod encoding`() {
        // xterm CSI 1;mod P: Shift=1, Alt=2, Ctrl=4 (modifier_code).
        assertArrayEquals(
            "\u001b[1;5P".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_F1,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[1;3Q".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_F2,
                0,
                ctrlActive = false,
                altActive = true,
            ),
        )
        assertArrayEquals(
            "\u001b[1;7P".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_F1,
                0,
                ctrlActive = true,
                altActive = true,
            ),
        )
        assertArrayEquals(
            "\u001b[15;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_F5,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent arrow with ctrl alt modifier combos`() {
        assertArrayEquals(
            "\u001b[1;5D".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_LEFT,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[1;7B".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_DOWN,
                0,
                ctrlActive = true,
                altActive = true,
            ),
        )
        // Modifier-carrying arrows stay CSI even in application cursor mode.
        assertArrayEquals(
            "\u001b[1;3A".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                0,
                ctrlActive = false,
                altActive = true,
                appCursorMode = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent ctrl enter produces csi 27 codepoint encoding`() {
        // ghostty function_keys.zig：带 Ctrl 的回车走 CSI u 编码
        // `CSI 27 ; mod ; 13 ~`（Ctrl=5，Ctrl+Alt=7），而非 `CSI 13;mod~`。
        assertArrayEquals(
            "\u001b[27;5;13~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_ENTER,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[27;7;13~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_ENTER,
                0,
                ctrlActive = true,
                altActive = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent ctrl alt tab encodes full modifier set`() {
        assertArrayEquals(
            "\u001b[9;7~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_TAB,
                0,
                ctrlActive = true,
                altActive = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent ctrl space sends null byte`() {
        // Some devices report unicodeChar=0 here, some 0x20 — both must fold.
        assertArrayEquals(
            bytes(0x00),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_SPACE,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            bytes(0x00),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_SPACE,
                0x20,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            bytes(0x1B, 0x00),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_SPACE,
                0,
                ctrlActive = true,
                altActive = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent ctrl shift letter folds case insensitive`() {
        // Ctrl+Shift+A reports unicodeChar='A'; fold must not care about case.
        assertArrayEquals(
            bytes(0x01),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_A,
                'A'.code,
                ctrlActive = true,
                altActive = false,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent del with modifiers uses csi 3 mod`() {
        assertArrayEquals(
            "\u001b[3;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DEL,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[3;3~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_DEL,
                0,
                ctrlActive = false,
                altActive = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent forward del and insert`() {
        assertArrayEquals(
            "\u001b[3~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_FORWARD_DEL, 0, false, false),
        )
        assertArrayEquals(
            "\u001b[3;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_FORWARD_DEL,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[2~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_INSERT, 0, false, false),
        )
        assertArrayEquals(
            "\u001b[2;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_INSERT,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent home and end with modifiers`() {
        assertArrayEquals(
            "\u001b[1;5H".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_MOVE_HOME,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[1;3F".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_MOVE_END,
                0,
                ctrlActive = false,
                altActive = true,
            ),
        )
        assertArrayEquals(
            "\u001b[5;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_PAGE_UP,
                0,
                ctrlActive = true,
                altActive = false,
            ),
        )
        assertArrayEquals(
            "\u001b[6;3~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(
                android.view.KeyEvent.KEYCODE_PAGE_DOWN,
                0,
                ctrlActive = false,
                altActive = true,
            ),
        )
    }

    @Test
    fun `encodeKeyEvent unknown key with printable char sends utf8`() {
        // A key with no special mapping falls through to plain character output.
        assertArrayEquals(
            bytes(0x78),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_X, 'x'.code, false, false),
        )
        assertArrayEquals(
            bytes(0x1B, 0x78),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_X, 'x'.code, false, altActive = true),
        )
    }

    @Test
    fun `ctrl punctuation folds via c and 1f`() {
        // Ctrl+/ → 0x0F, Ctrl+[ → ESC, Ctrl+2 → NUL (ANSI/VT100 tradition).
        assertArrayEquals(bytes(0x0F), enc("/", ctrl = true))
        assertArrayEquals(bytes(0x1B), enc("[", ctrl = true))
        assertArrayEquals(bytes(0x00), enc("2", ctrl = true))
    }

    @Test
    fun `cjk commit is utf8 passthrough`() {
        // 拼音候选上屏：单汉字与多汉字词组均按 UTF-8 直通，不得折叠。
        assertArrayEquals("中".toByteArray(Charsets.UTF_8), enc("中"))
        assertArrayEquals("中文测试".toByteArray(Charsets.UTF_8), enc("中文测试"))
    }

    @Test
    fun `cjk commit with ctrl held is not folded`() {
        // 粘滞 Ctrl 点亮时提交中文：CJK 码点无 Ctrl 映射，必须直通 UTF-8，
        // 不得丢弃或折叠为控制字节。
        assertArrayEquals("中".toByteArray(Charsets.UTF_8), enc("中", ctrl = true))
        assertArrayEquals("中文".toByteArray(Charsets.UTF_8), enc("中文", ctrl = true))
    }

    @Test
    fun `cjk commit with alt prefixes esc per code point`() {
        // Alt+中文：每码点前加 ESC（与西文 alt 路径一致的码点语义）。
        val expected = bytes(0x1B) + "中".toByteArray(Charsets.UTF_8)
        assertArrayEquals(expected, enc("中", alt = true))
    }

    @Test
    fun `every enter keycode produces carriage return`() {
        // 三个回车键码必须同义且都是 CR(0x0D)。载荷写 LF 时，raw 模式下的
        // TUI 应用会把 0x0A 读成 Ctrl+J 即字母 j；键码缺失时则落到 Bridge 的
        // KeyCharacterMap 猜测分支，同样表现为「回车变成某个字母」。
        listOf(
            android.view.KeyEvent.KEYCODE_ENTER,
            android.view.KeyEvent.KEYCODE_NUMPAD_ENTER,
            android.view.KeyEvent.KEYCODE_DPAD_CENTER,
        ).forEach { keyCode ->
            assertArrayEquals(
                "keyCode=$keyCode 必须产生 CR",
                bytes(0x0D),
                TerminalInputEncoder.encodeKeyEvent(keyCode, 0, false, false),
            )
        }
    }

    @Test
    fun `every enter keycode with ctrl reports csi 27 codepoint encoding`() {
        // 带 Ctrl 时同样按回车族统一上报 CSI u 编码。
        listOf(
            android.view.KeyEvent.KEYCODE_ENTER,
            android.view.KeyEvent.KEYCODE_NUMPAD_ENTER,
            android.view.KeyEvent.KEYCODE_DPAD_CENTER,
        ).forEach { keyCode ->
            assertArrayEquals(
                "keyCode=$keyCode 必须上报 CSI 27;5;13~",
                "\u001b[27;5;13~".toByteArray(Charsets.UTF_8),
                TerminalInputEncoder.encodeKeyEvent(keyCode, 0, ctrlActive = true, altActive = false),
            )
        }
    }

    @Test
    fun `ctrl j stays line feed`() {
        // Ctrl+J 语义上就是 LF，与回车无关：它必须仍是 0x0A，
        // 否则 Ctrl+J/Ctrl+Enter 在 shell 中将无法区分（POSIX 惯例）。
        assertArrayEquals(
            bytes(0x0A),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_J, 'j'.code, true, false),
        )
    }
}
