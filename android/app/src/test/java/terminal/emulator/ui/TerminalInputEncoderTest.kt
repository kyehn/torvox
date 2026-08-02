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
}
