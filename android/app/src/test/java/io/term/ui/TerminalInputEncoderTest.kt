package io.term.ui

import android.view.KeyEvent
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TerminalInputEncoderTest {
    @Test
    fun encodeCommittedText_preservesUtf8Characters() {
        assertArrayEquals(
            "λ好".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeCommittedText("λ好", ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeCommittedText_altPrefixesEachCharacterWithEscape() {
        assertArrayEquals(
            byteArrayOf(0x1B, 'a'.code.toByte(), 0x1B, 'b'.code.toByte()),
            TerminalInputEncoder.encodeCommittedText("ab", ctrlActive = false, altActive = true),
        )
    }

    @Test
    fun encodeCommittedText_ctrlLettersBecomeControlBytes() {
        assertArrayEquals(byteArrayOf(0x01, 0x1A), TerminalInputEncoder.encodeCommittedText("aZ", ctrlActive = true, altActive = false))
    }

    @Test
    fun encodeKeyEvent_preservesUtf8Characters() {
        assertArrayEquals(
            "好".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_UNKNOWN, '好'.code, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_altPrefixesUtf8CharacterWithEscape() {
        assertArrayEquals(
            byteArrayOf(0x1B) + "λ".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_UNKNOWN, 'λ'.code, ctrlActive = false, altActive = true),
        )
    }

    @Test
    fun encodeKeyEvent_ctrlKeyCodeWinsOverPrintableCharacter() {
        assertArrayEquals(
            byteArrayOf(0x03),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_C, 'c'.code, ctrlActive = true, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_arrowKeysUseVtSequences() {
        assertArrayEquals(
            "\u001b[A".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_UP, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_unknownWithoutUnicodeReturnsNull() {
        assertNull(TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_UNKNOWN, 0, ctrlActive = false, altActive = false))
    }

    @Test
    fun encodeKeyEvent_enterReturnsNewline() {
        assertArrayEquals(
            byteArrayOf(0x0A),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_ENTER, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_numpadEnterReturnsNewline() {
        assertArrayEquals(
            byteArrayOf(0x0A),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_NUMPAD_ENTER, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_homeKeyUsesEscapeH() {
        assertArrayEquals(
            "\u001b[H".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_MOVE_HOME, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_endKeyUsesEscapeF() {
        assertArrayEquals(
            "\u001b[F".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_MOVE_END, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_pageUpUsesEscape5Tilde() {
        assertArrayEquals(
            "\u001b[5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_PAGE_UP, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_pageDownUsesEscape6Tilde() {
        assertArrayEquals(
            "\u001b[6~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_PAGE_DOWN, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_ctrlAProduces0x01() {
        assertArrayEquals(
            byteArrayOf(0x01),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_A, 'a'.code, ctrlActive = true, altActive = false),
        )
    }

    @Test
    fun encodeCommittedText_ctrlNonLetterIgnored() {
        val result = TerminalInputEncoder.encodeCommittedText("1", ctrlActive = true, altActive = false)
        assertArrayEquals("1".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encodeCommittedText_ctrlAndAltCombined_ctrlTakesPriority() {
        assertArrayEquals(
            byteArrayOf(0x01),
            TerminalInputEncoder.encodeCommittedText("a", ctrlActive = true, altActive = true),
        )
    }

    @Test
    fun encodeCommittedText_emptyStringReturnsEmpty() {
        val result = TerminalInputEncoder.encodeCommittedText("", ctrlActive = false, altActive = false)
        assertArrayEquals(ByteArray(0), result)
    }

    @Test
    fun encodeKeyEvent_downArrowUsesEscapeB() {
        assertArrayEquals(
            "\u001b[B".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_DOWN, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_leftArrowUsesEscapeD() {
        assertArrayEquals(
            "\u001b[D".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_LEFT, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_rightArrowUsesEscapeC() {
        assertArrayEquals(
            "\u001b[C".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_RIGHT, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_backspaceSends0x7F() {
        assertArrayEquals(
            byteArrayOf(0x7F),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DEL, 0, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_tabSendsTab() {
        assertArrayEquals(
            byteArrayOf(0x09),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_TAB, 9, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_backspaceWithCtrlSendsCSI35() {
        assertArrayEquals(
            "\u001b[3;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DEL, 0, ctrlActive = true, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_ctrlArrowSendsModifierSequence() {
        val result = TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_UP, 0, ctrlActive = true, altActive = false)
        assertArrayEquals("\u001b[1;5A".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encodeKeyEvent_altArrowSendsModifierSequence() {
        val result = TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_UP, 0, ctrlActive = false, altActive = true)
        assertArrayEquals("\u001b[1;3A".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encodeKeyEvent_ctrlAltArrowSendsModifierSequence() {
        val result = TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_UP, 0, ctrlActive = true, altActive = true)
        assertArrayEquals("\u001b[1;7A".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encodeKeyEvent_ctrlArrowAllDirections() {
        assertArrayEquals(
            "\u001b[1;5A".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_UP, 0, ctrlActive = true, altActive = false),
        )
        assertArrayEquals(
            "\u001b[1;5B".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_DOWN, 0, ctrlActive = true, altActive = false),
        )
        assertArrayEquals(
            "\u001b[1;5C".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_RIGHT, 0, ctrlActive = true, altActive = false),
        )
        assertArrayEquals(
            "\u001b[1;5D".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_DPAD_LEFT, 0, ctrlActive = true, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_ctrlHomeEndPage() {
        assertArrayEquals(
            "\u001b[1;5H".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_MOVE_HOME, 0, ctrlActive = true, altActive = false),
        )
        assertArrayEquals(
            "\u001b[1;5F".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_MOVE_END, 0, ctrlActive = true, altActive = false),
        )
        assertArrayEquals(
            "\u001b[5;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_PAGE_UP, 0, ctrlActive = true, altActive = false),
        )
        assertArrayEquals(
            "\u001b[6;5~".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_PAGE_DOWN, 0, ctrlActive = true, altActive = false),
        )
    }

    @Test
    fun encodeCommittedText_bracketedPasteWrapsMultiChar() {
        val result = TerminalInputEncoder.encodeCommittedText("hello world", ctrlActive = false, altActive = false, bracketedPaste = true)
        assertArrayEquals("\u001b[200~hello world\u001b[201~".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encodeCommittedText_bracketedPasteDisabledForSingleChar() {
        val result = TerminalInputEncoder.encodeCommittedText("a", ctrlActive = false, altActive = false, bracketedPaste = true)
        assertArrayEquals("a".toByteArray(Charsets.UTF_8), result)
    }

    @Test
    fun encodeKeyEvent_numpadDivideWithUnicodeSendsCharacter() {
        assertArrayEquals(
            byteArrayOf('/'.code.toByte()),
            TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_NUMPAD_DIVIDE, '/'.code, ctrlActive = false, altActive = false),
        )
    }

    @Test
    fun encodeKeyEvent_forwardDelWithoutUnicodeReturnsNull() {
        val result = TerminalInputEncoder.encodeKeyEvent(KeyEvent.KEYCODE_FORWARD_DEL, 0, ctrlActive = false, altActive = false)
        assertNull("KEYCODE_FORWARD_DEL with no unicode should return null", result)
    }

    @Test
    fun encodeCommittedText_multiByteUtf8WithAlt() {
        val result = TerminalInputEncoder.encodeCommittedText("λ好", ctrlActive = false, altActive = true)
        val expected = byteArrayOf(0x1B) + "λ".toByteArray(Charsets.UTF_8) + byteArrayOf(0x1B) + "好".toByteArray(Charsets.UTF_8)
        assertArrayEquals(expected, result)
    }

    @Test
    fun encodeCommittedText_ctrlNumberPreservesNumber() {
        assertArrayEquals(
            "123".toByteArray(Charsets.UTF_8),
            TerminalInputEncoder.encodeCommittedText("123", ctrlActive = true, altActive = false),
        )
    }

    @Test
    fun encodeCommittedText_ctrlAltMixed_nonLetterPreservesWithAlt() {
        val result = TerminalInputEncoder.encodeCommittedText("1a", ctrlActive = true, altActive = true)
        // '1' has no control byte, so alt prefixes it with ESC
        // 'a' has control byte (0x01), and ctrl takes priority over alt
        assertArrayEquals(byteArrayOf(0x1B, 0x31, 0x01), result)
    }
}
