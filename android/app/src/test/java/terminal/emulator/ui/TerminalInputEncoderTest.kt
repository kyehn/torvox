package terminal.emulator.ui

import org.junit.Assert.assertArrayEquals
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
    fun `alt prefixes esc to every multi char commit`() {
        // Alt+drag/IME multi-char commit: xterm Meta prefixes each character.
        assertArrayEquals(bytes(0x1B, 0x61, 0x1B, 0x62), enc("ab", alt = true))
    }

    @Test
    fun `ctrl upper symbol row folds to the c0 controls`() {
        // 数字行的上档符号是 US 键盘上的另一排键，Ctrl 折叠必须落在各自的 C0 码上
        // 而不是各键的 `c & 0x1F`（那会把它们全折成 ESC）。
        // 期望取自 Android-Terminal-Emulator 谱系的既有字面断言
        // （jackpal/androidterm 的 TermKeyListenerTest，ThothTerm 沿用同一份）：
        // Ctrl+2→NUL、Ctrl+3→ESC、Ctrl+4→0x1C、Ctrl+5→0x1D、Ctrl+6→0x1E、
        // Ctrl+7→0x1F、Ctrl+8→DEL，以及 Ctrl+Space→NUL。
        val upperRow =
            mapOf(
                '2' to 0x00,
                '3' to 0x1B,
                '4' to 0x1C,
                '5' to 0x1D,
                '6' to 0x1E,
                '7' to 0x1F,
                '8' to 0x7F,
                ' ' to 0x00,
            )
        for ((char, expected) in upperRow) {
            assertArrayEquals("Ctrl+$char", bytes(expected), enc(char.toString(), ctrl = true))
        }
    }

    @Test
    fun `ctrl punctuation folds via c and 1f`() {
        // 标点行的折叠没有专用分支，走 `c & 0x1F`。逐键钉住，否则删掉或改写
        // 那个公式时不会有任何用例变红（上面的数字行有专用分支，不经过它）。
        // Ctrl+[→ESC、Ctrl+\→0x1C、Ctrl+]→0x1D、Ctrl+^→0x1E、Ctrl+_→0x1F、
        // Ctrl+/→0x0F。
        val punctuation =
            mapOf(
                '[' to 0x1B,
                '\\' to 0x1C,
                ']' to 0x1D,
                '^' to 0x1E,
                '_' to 0x1F,
                '/' to 0x0F,
            )
        for ((char, expected) in punctuation) {
            assertArrayEquals("Ctrl+$char", bytes(expected), enc(char.toString(), ctrl = true))
        }
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
}
