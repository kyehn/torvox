package io.term.ui

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
        text.forEach { character ->
            val controlByte = if (ctrlActive) controlByteForCharacter(character) else null
            if (controlByte != null) {
                bytes.add(controlByte)
            } else {
                if (altActive) bytes.add(0x1B)
                bytes.addAll(character.toString().toByteArray(Charsets.UTF_8).toList())
            }
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
            if (controlByte != null) return byteArrayOf(controlByte)
        }
        val escapeSequence = escapeSequenceForKeyCode(keyCode, ctrlActive, altActive)
        if (escapeSequence != null) return escapeSequence.toByteArray(Charsets.UTF_8)
        if (keyCode == KeyEvent.KEYCODE_ENTER || keyCode == KeyEvent.KEYCODE_NUMPAD_ENTER) return byteArrayOf(0x0A)
        if (keyCode == KeyEvent.KEYCODE_DEL) return byteArrayOf(0x7F)
        if (unicodeChar <= 0) return null
        val encoded = String(Character.toChars(unicodeChar)).toByteArray(Charsets.UTF_8)
        return if (altActive) byteArrayOf(0x1B) + encoded else encoded
    }

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

    private fun controlByteForCharacter(character: Char): Byte? = when (character) {
        in 'a'..'z' -> (character.code - LOWERCASE_CONTROL_OFFSET).toByte()
        in 'A'..'Z' -> (character.code - UPPERCASE_CONTROL_OFFSET).toByte()
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
