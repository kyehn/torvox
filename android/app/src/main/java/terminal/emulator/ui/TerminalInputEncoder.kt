package terminal.emulator.ui

import android.view.KeyEvent

object TerminalInputEncoder {
    private const val BRACKETED_PASTE_START = "\u001b[200~"
    private const val BRACKETED_PASTE_END = "\u001b[201~"
    private const val LOWERCASE_CONTROL_OFFSET = 96
    private const val UPPERCASE_CONTROL_OFFSET = 64

    fun encodeCommittedText(
        text: String,
        ctrlActive: Boolean,
        altActive: Boolean,
        bracketedPaste: Boolean = false,
    ): ByteArray {
        val bytes = mutableListOf<Byte>()
        if (bracketedPaste && text.length > 1) {
            bytes.addAll(BRACKETED_PASTE_START.toByteArray(Charsets.UTF_8).toList())
            bytes.addAll(text.toByteArray(Charsets.UTF_8).toList())
            bytes.addAll(BRACKETED_PASTE_END.toByteArray(Charsets.UTF_8).toList())
            return bytes.toByteArray()
        }
        // CTRL conversion applies to a single character only (a real Ctrl+X
        // keypress). Multi-character IME commits — pinyin candidates, swipe
        // input, IME-internal paste, autocomplete — must NOT have each
        // character folded into a control byte ("abc" → 0x01 0x02 0x03).
        // A bracketed-paste-wrapped multi-char commit is the correct shape.
        if (ctrlActive && text.length == 1) {
            val codePoint = text[0].code
            // Digits 1/9/0 have no traditional Ctrl mapping (c & 0x1F would
            // collide with Ctrl+Q / Ctrl+Y / Ctrl+P) and are dropped,
            // matching the hardware-key path (encodeKeyEvent).
            if (codePoint == '1'.code || codePoint == '9'.code || codePoint == '0'.code) {
                return byteArrayOf()
            }
            val controlByte = controlByteForCodePoint(codePoint)
            if (controlByte != null) return withAltPrefix(altActive, byteArrayOf(controlByte))
        }
        // Iterate by code point: text.forEach over Char would split surrogate
        // pairs and encode each half as U+FFFD replacement, corrupting any
        // supplementary-plane character (emoji) committed by the IME.
        var index = 0
        while (index < text.length) {
            val codePoint = text.codePointAt(index)
            if (altActive) bytes.add(0x1B)
            bytes.addAll(String(Character.toChars(codePoint)).toByteArray(Charsets.UTF_8).toList())
            index += Character.charCount(codePoint)
        }
        return bytes.toByteArray()
    }

    fun encodeKeyEvent(
        keyCode: Int,
        unicodeChar: Int,
        ctrlActive: Boolean,
        altActive: Boolean,
    ): ByteArray? {
        if (ctrlActive) {
            val controlByte = controlByteForKeyCode(keyCode)
            if (controlByte != null) return withAltPrefix(altActive, byteArrayOf(controlByte))
            // Ctrl+Space — some devices report unicodeChar=0 here, some 0x20.
            if (keyCode == KeyEvent.KEYCODE_SPACE) {
                return withAltPrefix(altActive, byteArrayOf(0x00))
            }
            // Fold Ctrl+printable-ASCII into a control byte (shared table
            // with encodeCommittedText). Ctrl+1/9/0 are not folded: no
            // traditional mapping, and c & 0x1F would collide with Ctrl+Q /
            // Ctrl+Y / Ctrl+P — they are dropped instead. When Alt is also
            // held the folded byte is prefixed with ESC, matching xterm
            // (Ctrl+Alt+A → ESC 0x01).
            if (unicodeChar in 0x20..0x7E) {
                if (unicodeChar == '1'.code || unicodeChar == '9'.code || unicodeChar == '0'.code) {
                    return null
                }
                val folded = controlByteForCodePoint(unicodeChar)
                if (folded != null) return withAltPrefix(altActive, byteArrayOf(folded))
            }
        }
        val escapeSequence = escapeSequenceForKeyCode(keyCode, ctrlActive, altActive)
        if (escapeSequence != null) return escapeSequence.toByteArray(Charsets.UTF_8)
        if (keyCode == KeyEvent.KEYCODE_ENTER || keyCode == KeyEvent.KEYCODE_NUMPAD_ENTER) return byteArrayOf(0x0A)
        if (keyCode == KeyEvent.KEYCODE_DEL) return byteArrayOf(0x7F)
        if (unicodeChar <= 0) return null
        val encoded = String(Character.toChars(unicodeChar)).toByteArray(Charsets.UTF_8)
        return if (altActive) byteArrayOf(0x1B) + encoded else encoded
    }

    /** Prefixes ESC when Alt is held, matching xterm (Alt+X → ESC x). */
    private fun withAltPrefix(altActive: Boolean, bytes: ByteArray): ByteArray = if (altActive) byteArrayOf(0x1B) + bytes else bytes

    private fun escapeSequenceForKeyCode(
        keyCode: Int,
        ctrlActive: Boolean,
        altActive: Boolean,
    ): String? {
        val hasModifier = ctrlActive || altActive
        if (hasModifier) {
            val csiSeq = csiSequenceWithModifier(keyCode, ctrlActive, altActive)
            if (csiSeq != null) return csiSeq
        }
        return when (keyCode) {
            KeyEvent.KEYCODE_TAB -> "\t"
            KeyEvent.KEYCODE_ESCAPE -> "\u001b"
            KeyEvent.KEYCODE_FORWARD_DEL -> "\u001b[3~"
            KeyEvent.KEYCODE_INSERT -> "\u001b[2~"
            KeyEvent.KEYCODE_F1 -> "\u001bOP"
            KeyEvent.KEYCODE_F2 -> "\u001bOQ"
            KeyEvent.KEYCODE_F3 -> "\u001bOR"
            KeyEvent.KEYCODE_F4 -> "\u001bOS"
            KeyEvent.KEYCODE_F5 -> "\u001b[15~"
            KeyEvent.KEYCODE_F6 -> "\u001b[17~"
            KeyEvent.KEYCODE_F7 -> "\u001b[18~"
            KeyEvent.KEYCODE_F8 -> "\u001b[19~"
            KeyEvent.KEYCODE_F9 -> "\u001b[20~"
            KeyEvent.KEYCODE_F10 -> "\u001b[21~"
            KeyEvent.KEYCODE_F11 -> "\u001b[23~"
            KeyEvent.KEYCODE_F12 -> "\u001b[24~"
            KeyEvent.KEYCODE_DPAD_UP -> "\u001b[A"
            KeyEvent.KEYCODE_DPAD_DOWN -> "\u001b[B"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "\u001b[C"
            KeyEvent.KEYCODE_DPAD_LEFT -> "\u001b[D"
            KeyEvent.KEYCODE_MOVE_HOME -> "\u001b[H"
            KeyEvent.KEYCODE_MOVE_END -> "\u001b[F"
            KeyEvent.KEYCODE_PAGE_UP -> "\u001b[5~"
            KeyEvent.KEYCODE_PAGE_DOWN -> "\u001b[6~"
            else -> null
        }
    }

    private fun csiSequenceWithModifier(
        keyCode: Int,
        ctrlActive: Boolean,
        altActive: Boolean,
    ): String? {
        val modifierParam = 1 + (if (altActive) 2 else 0) + (if (ctrlActive) 4 else 0)
        return when (keyCode) {
            KeyEvent.KEYCODE_F1 -> "\u001b[1;${modifierParam}P"
            KeyEvent.KEYCODE_F2 -> "\u001b[1;${modifierParam}Q"
            KeyEvent.KEYCODE_F3 -> "\u001b[1;${modifierParam}R"
            KeyEvent.KEYCODE_F4 -> "\u001b[1;${modifierParam}S"
            KeyEvent.KEYCODE_F5 -> "\u001b[15;$modifierParam~"
            KeyEvent.KEYCODE_F6 -> "\u001b[17;$modifierParam~"
            KeyEvent.KEYCODE_F7 -> "\u001b[18;$modifierParam~"
            KeyEvent.KEYCODE_F8 -> "\u001b[19;$modifierParam~"
            KeyEvent.KEYCODE_F9 -> "\u001b[20;$modifierParam~"
            KeyEvent.KEYCODE_F10 -> "\u001b[21;$modifierParam~"
            KeyEvent.KEYCODE_F11 -> "\u001b[23;$modifierParam~"
            KeyEvent.KEYCODE_F12 -> "\u001b[24;$modifierParam~"
            KeyEvent.KEYCODE_FORWARD_DEL -> "\u001b[3;$modifierParam~"
            KeyEvent.KEYCODE_INSERT -> "\u001b[2;$modifierParam~"
            KeyEvent.KEYCODE_DPAD_UP -> "\u001b[1;${modifierParam}A"
            KeyEvent.KEYCODE_DPAD_DOWN -> "\u001b[1;${modifierParam}B"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "\u001b[1;${modifierParam}C"
            KeyEvent.KEYCODE_DPAD_LEFT -> "\u001b[1;${modifierParam}D"
            KeyEvent.KEYCODE_MOVE_HOME -> "\u001b[1;${modifierParam}H"
            KeyEvent.KEYCODE_MOVE_END -> "\u001b[1;${modifierParam}F"
            KeyEvent.KEYCODE_PAGE_UP -> "\u001b[5;$modifierParam~"
            KeyEvent.KEYCODE_PAGE_DOWN -> "\u001b[6;$modifierParam~"
            KeyEvent.KEYCODE_DEL -> "\u001b[3;$modifierParam~"
            else -> null
        }
    }

    /**
     * POSIX Ctrl fold for a printable ASCII code point; null = not foldable
     * (callers send it verbatim or drop it). Digits 9/0 are intentionally
     * absent: c & 0x1F would collide with Ctrl+Y / Ctrl+P, so callers drop
     * them before calling.
     */
    private fun controlByteForCodePoint(codePoint: Int): Byte? = when (codePoint) {
        in 'a'.code..'z'.code -> (codePoint - LOWERCASE_CONTROL_OFFSET).toByte()

        in 'A'.code..'Z'.code -> (codePoint - UPPERCASE_CONTROL_OFFSET).toByte()

        // Space and digits 2-8 follow the ANSI/VT100 tradition (their shifted
        // symbols are @ [ \ ] ^ _): Ctrl+Space/Ctrl+2 → NUL, Ctrl+3 → ESC,
        // Ctrl+4 → 0x1C, Ctrl+5 → 0x1D, Ctrl+6 → 0x1E, Ctrl+7 → 0x1F,
        // Ctrl+8 → DEL.
        ' '.code, '2'.code -> 0x00

        '3'.code -> 0x1B

        '4'.code -> 0x1C

        '5'.code -> 0x1D

        '6'.code -> 0x1E

        '7'.code -> 0x1F

        '8'.code -> 0x7F

        // Ctrl+[ → ESC, Ctrl+\ → 0x1C, Ctrl+] → 0x1D, Ctrl+^ → 0x1E,
        // Ctrl+_ → 0x1F, Ctrl+/ → 0x0F, and other punctuation via c & 0x1F.
        in 0x20..0x7E -> (codePoint and 0x1F).toByte()

        else -> null
    }

    private fun controlByteForKeyCode(keyCode: Int): Byte? = when (keyCode) {
        KeyEvent.KEYCODE_A -> 0x01
        KeyEvent.KEYCODE_B -> 0x02
        KeyEvent.KEYCODE_C -> 0x03
        KeyEvent.KEYCODE_D -> 0x04
        KeyEvent.KEYCODE_E -> 0x05
        KeyEvent.KEYCODE_F -> 0x06
        KeyEvent.KEYCODE_G -> 0x07
        KeyEvent.KEYCODE_H -> 0x08
        KeyEvent.KEYCODE_I -> 0x09
        KeyEvent.KEYCODE_J -> 0x0A
        KeyEvent.KEYCODE_K -> 0x0B
        KeyEvent.KEYCODE_L -> 0x0C
        KeyEvent.KEYCODE_M -> 0x0D
        KeyEvent.KEYCODE_N -> 0x0E
        KeyEvent.KEYCODE_O -> 0x0F
        KeyEvent.KEYCODE_P -> 0x10
        KeyEvent.KEYCODE_Q -> 0x11
        KeyEvent.KEYCODE_R -> 0x12
        KeyEvent.KEYCODE_S -> 0x13
        KeyEvent.KEYCODE_T -> 0x14
        KeyEvent.KEYCODE_U -> 0x15
        KeyEvent.KEYCODE_V -> 0x16
        KeyEvent.KEYCODE_W -> 0x17
        KeyEvent.KEYCODE_X -> 0x18
        KeyEvent.KEYCODE_Y -> 0x19
        KeyEvent.KEYCODE_Z -> 0x1A
        else -> null
    }
}
