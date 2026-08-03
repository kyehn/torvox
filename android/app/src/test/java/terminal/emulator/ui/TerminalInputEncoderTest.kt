package terminal.emulator.ui

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TerminalInputEncoderTest {
    private fun enc(
        text: String,
        ctrl: Boolean = false,
        alt: Boolean = false,
        bracketed: Boolean = false,
    ): ByteArray = TerminalInputEncoder.encodeCommittedText(text, ctrl, alt, bracketed)

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
    fun `ctrl 1 9 0 are dropped`() {
        assertArrayEquals(byteArrayOf(), enc("1", ctrl = true))
        assertArrayEquals(byteArrayOf(), enc("9", ctrl = true))
        assertArrayEquals(byteArrayOf(), enc("0", ctrl = true))
    }

    @Test
    fun `alt prefixes esc`() {
        assertArrayEquals(bytes(0x1B, 0x61), enc("a", alt = true))
        assertArrayEquals(bytes(0x1B, 0x01), enc("a", ctrl = true, alt = true))
    }

    @Test
    fun `bracketed paste wraps multi char text`() {
        val wrapped = enc("hello", bracketed = true)
        val expected = bytes(0x1B, 0x5B, 0x32, 0x30, 0x30, 0x7E) +
            "hello".toByteArray(Charsets.UTF_8) +
            bytes(0x1B, 0x5B, 0x32, 0x30, 0x31, 0x7E)
        assertArrayEquals(expected, wrapped)
    }

    @Test
    fun `bracketed paste single char not wrapped`() {
        assertArrayEquals(bytes(0x61), enc("a", bracketed = true))
    }

    @Test
    fun `emoji surrogate pair survives`() {
        assertArrayEquals("😀".toByteArray(Charsets.UTF_8), enc("😀"))
    }

    @Test
    fun `encodeKeyEvent enter produces newline`() {
        assertArrayEquals(
            bytes(0x0A),
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
    fun `encodeKeyEvent ctrl c folds`() {
        assertArrayEquals(
            bytes(0x03),
            TerminalInputEncoder.encodeKeyEvent(android.view.KeyEvent.KEYCODE_C, 'c'.code, true, false),
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
    fun `encodeKeyEvent alt enter produces csi 13 mod`() {
        assertArrayEquals(
            bytes(0x1B, 0x5B, 0x31, 0x33, 0x3B, 0x33, 0x7E), // ESC [ 1 3 ; 3 ~
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
}
