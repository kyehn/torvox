package terminal.emulator.ui

import android.view.KeyEvent

object TerminalInputEncoder {
    private const val BRACKETED_PASTE_START = "\u001b[200~"
    private const val BRACKETED_PASTE_END = "\u001b[201~"
    private const val LOWERCASE_CONTROL_OFFSET = 96
    private const val UPPERCASE_CONTROL_OFFSET = 64

    /**
     * 回车键的全部键码，含导航键中心（`KEYCODE_DPAD_CENTER`，部分输入法与遥控器
     * 以它代替 Enter 提交）。
     *
     * 单一来源：缺失任一键码都会让它落到 [Bridge.processKeyEvent] 的
     * `KeyCharacterMap` 猜测分支——该分支对导航键无定义，会回退成任意字符
     * （实测 DPAD_CENTER 提交后无换行，文本与下一条命令被粘连）。回车被当作
     * 粘滞 Ctrl 之外的普通可打印键送出时，即表现为「回车变成某个字母」。
     */
    private val ENTER_KEY_CODES =
        setOf(KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_DPAD_CENTER)

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
        // Ctrl 转换只适用于单个字符（即真实的 Ctrl+X 按键）。
        // 多字符输入法提交——拼音候选、滑行输入、输入法内部粘贴、自动补全
        // ——绝不能逐字符折叠为控制字节（"abc" → 0x01 0x02 0x03）。
        // 正确形态是由 bracketed-paste 包裹的多字符提交。
        if (ctrlActive && text.length == 1) {
            val codePoint = text[0].code
            // 数字 1/9/0 没有传统的 Ctrl 映射（c & 0x1F 会与
            // Ctrl+Q / Ctrl+Y / Ctrl+P 冲突）；按 zed 的 mappings/keys.rs，
            // 它们改以 `CSI 27;5;code~` 发出而非被丢弃，
            // 与硬件按键路径一致。
            if (codePoint == '1'.code || codePoint == '9'.code || codePoint == '0'.code) {
                val modifier = 1 + (if (altActive) 2 else 0) + 4
                return csi27(modifier, codePoint)
            }
            val controlByte = controlByteForCodePoint(codePoint)
            if (controlByte != null) return withAltPrefix(altActive, byteArrayOf(controlByte))
        }
        // 按码点迭代：text.forEach 遍历 Char 会拆开代理对，
        // 并把每一半编码为 U+FFFD 替换字符，破坏输入法提交的任何
        // 增补平面字符（emoji）。
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
        appCursorMode: Boolean = false,
    ): ByteArray? {
        if (ctrlActive) {
            val controlByte = controlByteForKeyCode(keyCode)
            if (controlByte != null) return withAltPrefix(altActive, byteArrayOf(controlByte))
            // Ctrl+Space——部分设备在此报告 unicodeChar=0，部分报告 0x20。
            if (keyCode == KeyEvent.KEYCODE_SPACE) {
                return withAltPrefix(altActive, byteArrayOf(0x00))
            }
            // 把 Ctrl+可打印 ASCII 折叠为控制字节（与 encodeCommittedText 共用表）。
            // 数字 1/9/0 没有传统映射（c & 0x1F 会与 Ctrl+Q / Ctrl+Y / Ctrl+P 冲突），
            // 故按 zed 的 mappings/keys.rs 改以 `CSI 27;5;code~` 发出而非丢弃。
            // 同时按住 Alt 时折叠字节前加 ESC 前缀，对应 xterm
            // （Ctrl+Alt+A → ESC 0x01）。
            if (unicodeChar in 0x20..0x7E) {
                if (unicodeChar == '1'.code || unicodeChar == '9'.code || unicodeChar == '0'.code) {
                    val modifier = 1 + (if (altActive) 2 else 0) + 4
                    return csi27(modifier, unicodeChar)
                }
                val folded = controlByteForCodePoint(unicodeChar)
                if (folded != null) return withAltPrefix(altActive, byteArrayOf(folded))
            }
        }
        val escapeSequence = escapeSequenceForKeyCode(keyCode, ctrlActive, altActive, appCursorMode)
        if (escapeSequence != null) return escapeSequence.toByteArray(Charsets.UTF_8)
        if (keyCode in ENTER_KEY_CODES) return byteArrayOf(0x0A)
        if (keyCode == KeyEvent.KEYCODE_DEL) return byteArrayOf(0x7F)
        if (unicodeChar <= 0) return null
        val encoded = String(Character.toChars(unicodeChar)).toByteArray(Charsets.UTF_8)
        return if (altActive) byteArrayOf(0x1B) + encoded else encoded
    }

    /**
     * xterm/zed 的 `CSI 27` 修饰键编码：`ESC [ 27 ; modifier ; code ~`。
     * 修饰键位：Shift=1，Alt=2，Ctrl=4。用于没有传统脱字符映射的
     * Ctrl+数字（Ctrl+数字/标点 → `CSI 27;5;n~`）。
     */
    private fun csi27(modifier: Int, code: Int): ByteArray = "\u001b[27;$modifier;$code~".toByteArray(Charsets.UTF_8)

    /** 按住 Alt 时前置 ESC，与 xterm 一致（Alt+X → ESC x）。 */
    private fun withAltPrefix(altActive: Boolean, bytes: ByteArray): ByteArray = if (altActive) {
        byteArrayOf(
            0x1B,
        ) + bytes
    } else {
        bytes
    }

    private fun escapeSequenceForKeyCode(
        keyCode: Int,
        ctrlActive: Boolean,
        altActive: Boolean,
        appCursorMode: Boolean,
    ): String? {
        val hasModifier = ctrlActive || altActive
        if (hasModifier) {
            val csiSeq = csiSequenceWithModifier(keyCode, ctrlActive, altActive)
            if (csiSeq != null) return csiSeq
        }
        return when (keyCode) {
            KeyEvent.KEYCODE_TAB ->
                when {
                    // 按住 Alt 时，xterm 发送 ESC TAB（Meta 前缀）而非裸制表符；
                    // 按住 Ctrl 时发送 CSI 9;mod~（xterm/kitty 约定）。
                    ctrlActive || altActive -> {
                        val modParam = 1 + (if (altActive) 2 else 0) + (if (ctrlActive) 4 else 0)
                        "\u001b[9;$modParam~"
                    }

                    else -> "\t"
                }

            in ENTER_KEY_CODES ->
                if (ctrlActive || altActive) {
                    // 带修饰键的回车：xterm 经 CSI 13;mod~ 上报。
                    val modParam = 1 + (if (altActive) 2 else 0) + (if (ctrlActive) 4 else 0)
                    "\u001b[13;$modParam~"
                } else {
                    "\n"
                }

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

            KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN,
            KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_DPAD_LEFT,
            ->
                arrowSequence(keyCode, appCursorMode)

            KeyEvent.KEYCODE_MOVE_HOME -> "\u001b[H"

            KeyEvent.KEYCODE_MOVE_END -> "\u001b[F"

            KeyEvent.KEYCODE_PAGE_UP -> "\u001b[5~"

            KeyEvent.KEYCODE_PAGE_DOWN -> "\u001b[6~"

            else -> null
        }
    }

    private fun csiSequenceWithModifier(keyCode: Int, ctrlActive: Boolean, altActive: Boolean): String? {
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
     * 遵循 DECCKM 的方向键序列（见 `openspec/specs/modifier-bar-arrow-encoding/spec.md`）：
     * 在应用光标模式下方向键须用 SS3（`ESC O A`）而非 CSI（`ESC [ A`），
     * 否则 app 模式下的 vim/less/mutt 会误读。
     * 带修饰键的方向键不会到达此辅助函数——它们由 [csiSequenceWithModifier] 处理。
     * 与 [ModifierBar] 共用，其方向键按钮必须遵循相同模式。
     */
    internal fun arrowSequence(keyCode: Int, appCursorMode: Boolean): String = when {
        appCursorMode && keyCode == KeyEvent.KEYCODE_DPAD_UP -> "\u001bOA"
        appCursorMode && keyCode == KeyEvent.KEYCODE_DPAD_DOWN -> "\u001bOB"
        appCursorMode && keyCode == KeyEvent.KEYCODE_DPAD_RIGHT -> "\u001bOC"
        appCursorMode && keyCode == KeyEvent.KEYCODE_DPAD_LEFT -> "\u001bOD"
        keyCode == KeyEvent.KEYCODE_DPAD_UP -> "\u001b[A"
        keyCode == KeyEvent.KEYCODE_DPAD_DOWN -> "\u001b[B"
        keyCode == KeyEvent.KEYCODE_DPAD_RIGHT -> "\u001b[C"
        else -> "\u001b[D"
    }

    /**
     * 可打印 ASCII 码点的 POSIX Ctrl 折叠；null = 不可折叠（调用方原样发送或丢弃）。
     * 刻意不含数字 9/0：c & 0x1F 会与 Ctrl+Y / Ctrl+P 冲突，调用方在调用前先丢弃它们。
     */
    private fun controlByteForCodePoint(codePoint: Int): Byte? = when (codePoint) {
        in 'a'.code..'z'.code -> (codePoint - LOWERCASE_CONTROL_OFFSET).toByte()

        in 'A'.code..'Z'.code -> (codePoint - UPPERCASE_CONTROL_OFFSET).toByte()

        // 空格与数字 2-8 遵循 ANSI/VT100 传统（其上档符号为 @ [ \ ] ^ _）：
        // Ctrl+Space/Ctrl+2 → NUL，Ctrl+3 → ESC，Ctrl+4 → 0x1C，Ctrl+5 → 0x1D，
        // Ctrl+6 → 0x1E，Ctrl+7 → 0x1F，Ctrl+8 → DEL。
        ' '.code, '2'.code -> 0x00

        '3'.code -> 0x1B

        '4'.code -> 0x1C

        '5'.code -> 0x1D

        '6'.code -> 0x1E

        '7'.code -> 0x1F

        '8'.code -> 0x7F

        // Ctrl+[ → ESC，Ctrl+\ → 0x1C，Ctrl+] → 0x1D，Ctrl+^ → 0x1E，
        // Ctrl+_ → 0x1F，Ctrl+/ → 0x0F，其他标点经 c & 0x1F 处理。
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
