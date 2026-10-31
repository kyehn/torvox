package terminal.emulator.settings

import android.view.KeyEvent
import terminal.emulator.input.KeyModifiers

/**
 * `extra-keys` 宏（`{macro: "CTRL f d"}`）的一步。
 *
 * 上游 termux 的宏语义：宏串按空白切成若干记号，`CTRL`/`ALT`/`SHIFT` 是**修饰记号**，
 * 作用于其后第一个普通记号（用过即清）；普通记号是键名或一段字面量文本。上游实现见
 * `TerminalExtraKeys.onExtraKeyButtonClick`。
 *
 * 本应用把有键码的普通记号解析成 [MacroStep.Key]，其余按字面量文本
 * （[MacroStep.Text]）直接写出；修饰位沿用上游 `key::Mods` 的原始位
 * （1=shift，2=ctrl，4=alt，8=super）。
 */
sealed interface MacroStep {
    /** 修饰记号：置位后作用于下一个普通键。 */
    data class Modifier(val bit: Int) : MacroStep

    /**
     * 普通记号：Android 键码。
     *
     * [token] 保留用户写的原样（用于日志与键帽文本），键码才是真正发出去的东西。
     */
    data class Key(val keyCode: Int, val token: String) : MacroStep

    /** 普通记号：没有键码的字面量文本（`-_-`、`é` 等），按上游 `inputCodePoint` 写出。 */
    data class Text(val text: String) : MacroStep
}

/**
 * 宏串 → 步骤序列。
 *
 * 解析不出来时返回 null，调用方据此**不渲染**这个键：渲染一个点了没反应的键，
 * 比不渲染更糟（用户无从判断是配置写错还是应用坏了）。
 *
 * 键名（含上游别名表）走键码；其余记号按上游语义是字面量文本。唯一不支持的是
 * `FN`（本应用没有 FN 映射），含它的宏整体不可用。
 */
object MacroParser {
    /** 宏里的修饰记号 → 上游 `key::Mods` 位。 */
    private val modifierTokens: Map<String, Int> =
        mapOf(
            "CTRL" to KeyModifiers.CTRL,
            "ALT" to KeyModifiers.ALT,
            "SHIFT" to KeyModifiers.SHIFT,
        )

    /**
     * 单字符 → Android 键码。
     *
     * 只收原生键表里有的键（`native/src/android/keymap.rs`）：键码不认识时，
     * 编码器无从知道该按哪个逻辑键发，字符会在编码阶段被丢掉。
     */
    private val characterKeyCodes: Map<Char, Int> =
        mapOf(
            ' ' to KeyEvent.KEYCODE_SPACE,
            '-' to KeyEvent.KEYCODE_MINUS,
            '=' to KeyEvent.KEYCODE_EQUALS,
            '[' to KeyEvent.KEYCODE_LEFT_BRACKET,
            ']' to KeyEvent.KEYCODE_RIGHT_BRACKET,
            '\\' to KeyEvent.KEYCODE_BACKSLASH,
            ';' to KeyEvent.KEYCODE_SEMICOLON,
            '\'' to KeyEvent.KEYCODE_APOSTROPHE,
            '`' to KeyEvent.KEYCODE_GRAVE,
            ',' to KeyEvent.KEYCODE_COMMA,
            '.' to KeyEvent.KEYCODE_PERIOD,
            '/' to KeyEvent.KEYCODE_SLASH,
        )

    /**
     * 解析宏串；不可用时返回 null。
     *
     * 空宏、只有修饰记号的宏、含 `FN` 的宏都判为不可用：前两者没有可发的东西，
     * 后者需要本应用没有的 FN 二次映射。
     */
    fun parse(macro: String): List<MacroStep>? {
        val steps = mutableListOf<MacroStep>()
        for (token in tokens(macro)) {
            val normalized = TerminalKeyCatalog.normalizeKeyName(token)
            val modifier = modifierTokens[normalized]
            if (modifier != null) {
                steps += MacroStep.Modifier(modifier)
                continue
            }
            // FN 需要一整套二次映射（本应用没有），宏里出现它就整体不可用。
            if (normalized == "FN") return null
            val keyCode = TerminalKeyCatalog.keyCodeFor(normalized) ?: characterKeyCode(normalized)
            if (keyCode == null) {
                // 没有键码的记号是字面量文本（上游 inputCodePoint 逐码点写出）。
                steps += MacroStep.Text(token)
                continue
            }
            // 大写字母是字面量（上游按 code point 发送），键码本身不区分大小写；
            // 补一个 SHIFT 才能让编码器产出大写，否则 `{macro: "A"}` 发的是 `a`。
            if (token.length == 1 && token[0] in 'A'..'Z') {
                steps += MacroStep.Modifier(KeyModifiers.SHIFT)
            }
            steps += MacroStep.Key(keyCode, token)
        }
        return steps.takeIf { steps -> steps.any { it is MacroStep.Key || it is MacroStep.Text } }
    }

    /** 宏记号（与 [parse] 同一套切分规则）。 */
    private fun tokens(macro: String): List<String> = macro.split(' ', '\t', '\n', '\r').filter { it.isNotEmpty() }

    /**
     * 宏键帽的默认文本（没有显式 `display` 时）。
     *
     * 上游把每个记号先套别名、再查显示表，最后用空格拼回
     * （`ExtraKeyButton` 的构造函数）。显式 `display` 不经过这里。
     */
    fun display(macro: String, style: ExtraKeysStyle, allCaps: Boolean): String =
        tokens(macro).joinToString(" ") { TerminalKeyCatalog.displayLabel(it, style, allCaps) }

    /**
     * 宏里一段字面量文本按上游 `TerminalView.inputCodePoint` 编码后的字节。
     *
     * Ctrl 折叠的是整段文本的每个码点（`CTRL foo` 三个字母各折一次），Alt 在每个
     * 码点前加 ESC；Shift 对字面量没有作用（上游同样忽略）。UTF-8 编码保证非 ASCII
     * 字面量与终端收到的字节一致。
     */
    fun encodeLiteral(text: String, modifiers: Int): ByteArray {
        val ctrl = modifiers and KeyModifiers.CTRL != 0
        val alt = modifiers and KeyModifiers.ALT != 0
        val output = StringBuilder(text.length)
        var index = 0
        while (index < text.length) {
            val codePoint = text.codePointAt(index)
            index += Character.charCount(codePoint)
            if (alt) output.append('\u001B')
            output.appendCodePoint(if (ctrl) foldControl(codePoint) else codePoint)
        }
        return output.toString().toByteArray(Charsets.UTF_8)
    }

    /** 上游 `TerminalView.inputCodePoint` 的 Ctrl 折叠表；表外的码点原样保留。 */
    private fun foldControl(codePoint: Int): Int = when {
        codePoint in 'a'.code..'z'.code -> codePoint - 'a'.code + 1
        codePoint in 'A'.code..'Z'.code -> codePoint - 'A'.code + 1
        codePoint == ' '.code || codePoint == '2'.code -> 0
        codePoint == '['.code || codePoint == '3'.code -> 27
        codePoint == '\\'.code || codePoint == '4'.code -> 28
        codePoint == ']'.code || codePoint == '5'.code -> 29
        codePoint == '^'.code || codePoint == '6'.code -> 30
        codePoint == '_'.code || codePoint == '7'.code || codePoint == '/'.code -> 31
        codePoint == '8'.code -> 127
        else -> codePoint
    }

    /** 单字符记号的键码；不是本应用能编码的字符时为 null。 */
    private fun characterKeyCode(token: String): Int? {
        if (token.length != 1) return null
        val character = token[0]
        return when {
            character in 'a'..'z' -> KeyEvent.KEYCODE_A + (character - 'a')
            character in 'A'..'Z' -> KeyEvent.KEYCODE_A + (character - 'A')
            character in '0'..'9' -> KeyEvent.KEYCODE_0 + (character - '0')
            else -> characterKeyCodes[character]
        }
    }
}
