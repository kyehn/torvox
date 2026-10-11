package terminal.emulator.ui

import android.view.KeyEvent

/**
 * 按键到 PTY 字节的编码入口。
 *
 * **硬件按键与工具栏按键不走此处**：它们经 `Bridge.processKeyEvent` →
 * 原生 `key::Encoder` 编码，DECCKM、modifyOtherKeys、kitty 键盘协议、
 * backarrow key mode 一律由终端状态决定。
 *
 * 此处只保留输入法提交路径：输入法给出的是**文本**而非按键，没有可编码的
 * 逻辑键，只能按文本本身处理。
 */
object TerminalInputEncoder {
    /**
     * 回车字节（CR，0x0D）。终端的回车**不是** LF：raw 模式下的 TUI 应用
     * 按控制字符把 0x0A 解成 Ctrl+J，即字母 j。
     */
    private const val CARRIAGE_RETURN_BYTE: Byte = 0x0D

    private const val LOWERCASE_CONTROL_OFFSET = 96
    private const val UPPERCASE_CONTROL_OFFSET = 64

    /**
     * xterm 的 `modifier_code`（ghostty `modifier_code`、kitty、wezterm termwiz 一致）：
     * 无修饰为 1，Shift 加 1，Alt 加 2，Ctrl 加 4。
     *
     * 只有输入法提交路径经此算式：它拿不到按键的 Shift 位（Shift 已在提交文本里），
     * 故参数只有 Ctrl 与 Alt。
     */
    private const val UNMODIFIED_MODIFIER_PARAM = 1

    private fun modifierParam(ctrlActive: Boolean, altActive: Boolean): Int =
        UNMODIFIED_MODIFIER_PARAM + (if (altActive) 2 else 0) + (if (ctrlActive) 4 else 0)

    /**
     * 回车键的全部键码，含导航键中心（`KEYCODE_DPAD_CENTER`，部分输入法与遥控器
     * 以它代替 Enter 提交）。
     *
     * 单一来源：贴底判定经此集合。缺失任一键码都会让它落到
     * [terminal.emulator.bridge.Bridge.processKeyEvent] 的 `KeyCharacterMap` 猜测
     * 分支——该分支对导航键无定义，会回退成任意字符（实测提交后无换行，
     * 文本与下一条命令粘连）。
     */
    // 这三个是全部表示回车的公开码：主回车、小键盘回车、方向键中心。
    // （`KEYCODE_ISO_ENTER` 是 SDK 隐藏常量，编译期不可用；它只在 ISO 键盘
    // 硬件上出现，Android 自带的虚拟键盘不发。）
    val enterKeyCodes: Set<Int> =
        setOf(KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_DPAD_CENTER)

    /**
     * 输入法提交文本 → PTY 字节。
     *
     * 输入法的回车键走 `commitText("\n")`（多行字段）或 `sendKeyEvent(ENTER)`
     * 两条路径，前者必须在此归一为 CR(0x0D)：终端的回车是 CR，LF 在 raw 模式的
     * TUI 应用（helix/vim/less）里被读成 Ctrl+J 即字母 j。
     *
     * 只归一**孤立**的 `"\n"`：多字符提交（拼音候选、滑行输入、输入法内部粘贴）
     * 里的换行逐字保留，否则多行粘贴会粘成一行。注意这不是 bracketed paste——
     * 输入法提交路径不经按键编码器，多行提交在 helix 里**仍是 Ctrl+J**；真正该由
     * 粘贴路径解决的 bracketed paste 尚未实现，此处不假装覆盖。
     */
    fun encodeCommittedText(text: String, ctrlActive: Boolean, altActive: Boolean): ByteArray {
        val bytes = mutableListOf<Byte>()
        // 修饰键状态不参与归一：输入法提交的换行没有可用的修饰语义（IME 只给出
        // 一个 `"\n"`，无从得知按了 Ctrl+Alt），故只能按裸回车发出。这与硬件回车
        // 路径按修饰键发 CSI 序列不同，但后者拿得到修饰信息，不构成同一手势的分叉。
        if (text == "\n") return byteArrayOf(CARRIAGE_RETURN_BYTE)
        // Ctrl 转换只适用于单个字符（即真实的 Ctrl+X 按键）。
        // 多字符输入法提交——拼音候选、滑行输入、输入法内部粘贴、自动补全
        // ——绝不能逐字符折叠为控制字节（"abc" → 0x01 0x02 0x03），原样逐码点送出。
        if (ctrlActive && text.length == 1) {
            val codePoint = text[0].code
            // 数字 1/9/0 没有传统的 Ctrl 映射（c & 0x1F 会与
            // Ctrl+Q / Ctrl+Y / Ctrl+P 冲突）；按 zed 的 mappings/keys.rs，
            // 它们改以 `CSI 27;5;code~` 发出而非被丢弃。
            if (codePoint == '1'.code || codePoint == '9'.code || codePoint == '0'.code) {
                return csi27(modifierParam(ctrlActive = true, altActive = altActive), codePoint)
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
}
