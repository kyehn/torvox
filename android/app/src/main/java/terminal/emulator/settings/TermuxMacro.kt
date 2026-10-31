package terminal.emulator.settings

import android.view.KeyEvent
import terminal.emulator.input.KeyModifiers

/**
 * `extra-keys` 宏（`{macro: "CTRL f d"}`）的一步。
 *
 * 上游 termux 的宏语义：宏串按空白切成若干记号，`CTRL`/`ALT`/`SHIFT` 是**修饰记号**，
 * 作用于其后第一个普通记号（用过即清），普通记号是键名或单字符。上游实现见
 * `TerminalExtraKeys.onExtraKeyButtonClick`。
 *
 * 本应用把普通记号解析成 Android 键码（[MacroStep.Key]），修饰位沿用上游
 * `key::Mods` 的原始位（1=shift，2=ctrl，4=alt，8=super）。
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
}

/**
 * 宏串 → 步骤序列。
 *
 * 解析不出来时返回 null，调用方据此**不渲染**这个键：渲染一个点了没反应的键，
 * 比不渲染更糟（用户无从判断是配置写错还是应用坏了）。
 *
 * 只收「能确定发出什么」的记号：键名（含上游别名表）与单字符（字母、数字、
 * 常用标点）。其余记号——尤其是 `FN`（本应用没有 FN 映射）——一律判为不可用。
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
     * 空宏、只有修饰记号的宏、含未知记号的宏都判为不可用：前者没有可发的东西，
     * 后者发出去的一半与用户预期不符。
     */
    fun parse(macro: String): List<MacroStep>? {
        val tokens = macro.split(' ', '\t', '\n', '\r').filter { it.isNotEmpty() }
        if (tokens.isEmpty()) return null
        val steps = mutableListOf<MacroStep>()
        for (token in tokens) {
            val normalized = TerminalKeyCatalog.normalizeKeyName(token)
            val modifier = modifierTokens[normalized]
            if (modifier != null) {
                steps += MacroStep.Modifier(modifier)
                continue
            }
            // FN 需要一整套二次映射（本应用没有），宏里出现它就整体不可用。
            if (normalized == "FN") return null
            val keyCode = TerminalKeyCatalog.keyCodeFor(normalized) ?: characterKeyCode(normalized)
            if (keyCode == null) return null
            // 大写字母是字面量（上游按 code point 发送），键码本身不区分大小写；
            // 补一个 SHIFT 才能让编码器产出大写，否则 `{macro: "A"}` 发的是 `a`。
            if (token.length == 1 && token[0] in 'A'..'Z') {
                steps += MacroStep.Modifier(KeyModifiers.SHIFT)
            }
            steps += MacroStep.Key(keyCode, token)
        }
        return steps.takeIf { steps -> steps.any { it is MacroStep.Key } }
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
