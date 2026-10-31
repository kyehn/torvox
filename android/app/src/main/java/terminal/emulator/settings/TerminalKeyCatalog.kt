package terminal.emulator.settings

import android.view.KeyEvent

/**
 * termux `extra-keys` 键名 → Android 键码 / 显示字符。
 *
 * 键名表逐项对齐 termux-app `ExtraKeysConstants.PRIMARY_KEY_CODES_FOR_STRINGS`；
 * 少一项的表现是「自定义布局里那个键点了没反应」，故用测试锁住集合。
 */
object TerminalKeyCatalog {
    /** 特殊按键：不产生键码，由键栏本身处理（修饰键、抽屉、键盘开关等）。 */
    enum class Special { CTRL, ALT, FN, DRAWER, SCROLL, KEYBOARD, PASTE }

    private val keyCodes: Map<String, Int> =
        mapOf(
            "SPACE" to KeyEvent.KEYCODE_SPACE,
            "ESC" to KeyEvent.KEYCODE_ESCAPE,
            "TAB" to KeyEvent.KEYCODE_TAB,
            "HOME" to KeyEvent.KEYCODE_MOVE_HOME,
            "END" to KeyEvent.KEYCODE_MOVE_END,
            "PGUP" to KeyEvent.KEYCODE_PAGE_UP,
            "PGDN" to KeyEvent.KEYCODE_PAGE_DOWN,
            "INS" to KeyEvent.KEYCODE_INSERT,
            "DEL" to KeyEvent.KEYCODE_FORWARD_DEL,
            "BKSP" to KeyEvent.KEYCODE_DEL,
            "UP" to KeyEvent.KEYCODE_DPAD_UP,
            "LEFT" to KeyEvent.KEYCODE_DPAD_LEFT,
            "RIGHT" to KeyEvent.KEYCODE_DPAD_RIGHT,
            "DOWN" to KeyEvent.KEYCODE_DPAD_DOWN,
            "ENTER" to KeyEvent.KEYCODE_ENTER,
            "F1" to KeyEvent.KEYCODE_F1,
            "F2" to KeyEvent.KEYCODE_F2,
            "F3" to KeyEvent.KEYCODE_F3,
            "F4" to KeyEvent.KEYCODE_F4,
            "F5" to KeyEvent.KEYCODE_F5,
            "F6" to KeyEvent.KEYCODE_F6,
            "F7" to KeyEvent.KEYCODE_F7,
            "F8" to KeyEvent.KEYCODE_F8,
            "F9" to KeyEvent.KEYCODE_F9,
            "F10" to KeyEvent.KEYCODE_F10,
            "F11" to KeyEvent.KEYCODE_F11,
            "F12" to KeyEvent.KEYCODE_F12,
        )

    private val specials: Map<String, Special> =
        mapOf(
            "CTRL" to Special.CTRL,
            "ALT" to Special.ALT,
            "FN" to Special.FN,
            "DRAWER" to Special.DRAWER,
            "SCROLL" to Special.SCROLL,
            "KEYBOARD" to Special.KEYBOARD,
            "PASTE" to Special.PASTE,
        )

    /** termux `PRIMARY_REPETITIVE_KEYS`：按住即连发。 */
    private val repetitive: Set<String> = setOf("UP", "DOWN", "LEFT", "RIGHT", "BKSP", "DEL", "PGUP", "PGDN")

    /**
     * termux `ExtraKeysConstants.CONTROL_CHARS_ALIASES`：键名别名。
     *
     * 上游在解析每个键名时先套这张表（`ExtraKeyButton` 构造函数的 `replaceAlias`），
     * 故 `ESCAPE` 与 `ESC`、`CONTROL` 与 `CTRL` 在上游等价。少了这一步，这些写法会被
     * 当成未知键名丢掉——同一份 `extra-keys` 在两个应用里画出的键栏不一样。
     */
    private val aliases: Map<String, String> =
        mapOf(
            "ESCAPE" to "ESC",
            "CONTROL" to "CTRL",
            "SHFT" to "SHIFT",
            "RETURN" to "ENTER",
            "FUNCTION" to "FN",
            // 方向键常写成首尾字母
            "LT" to "LEFT",
            "RT" to "RIGHT",
            "DN" to "DOWN",
            "PAGEUP" to "PGUP",
            "PAGE_UP" to "PGUP",
            "PAGE UP" to "PGUP",
            "PAGE-UP" to "PGUP",
            "PAGEDOWN" to "PGDN",
            "PAGE_DOWN" to "PGDN",
            "PAGE-DOWN" to "PGDN",
            "DELETE" to "DEL",
            "BACKSPACE" to "BKSP",
            // 便于在 termux.properties 里写这三个字符
            "BACKSLASH" to "\\",
            "QUOTE" to "\"",
            "APOSTROPHE" to "'",
        )

    /**
     * 归一键名：先大写，再套用上游别名表。
     *
     * 大写是本应用的宽松之处（上游的别名表按原文精确匹配），但归一后的名字才是
     * 「这个键到底是什么」的答案：别名指向单字符时（`BACKSLASH` → `\`）尤其重要——
     * 按原名判会把它当成未知键名丢掉，按归一名判才是字面量。
     */
    fun normalizeKeyName(name: String): String {
        val upper = name.uppercase()
        return aliases[upper] ?: upper
    }

    fun keyCodeFor(name: String): Int? = keyCodes[normalizeKeyName(name)]

    fun specialFor(name: String): Special? = specials[normalizeKeyName(name)]

    fun isRepetitive(name: String): Boolean = normalizeKeyName(name) in repetitive

    /** 各样式下 termux 使用的显示字符。样式只改外观，不改按键本身。 */
    private val arrows = mapOf("LEFT" to "←", "RIGHT" to "→", "UP" to "↑", "DOWN" to "↓")
    private val wellKnown =
        mapOf(
            "ENTER" to "↲",
            "TAB" to "↹",
            "BKSP" to "⌫",
            "DEL" to "⌦",
            "DRAWER" to "☰",
            "KEYBOARD" to "⌨",
            "PASTE" to "⎘",
            "SCROLL" to "⇳",
        )
    private val lessKnown = mapOf("HOME" to "⇱", "END" to "⇲", "PGUP" to "⇑", "PGDN" to "⇓")
    private val nicer = mapOf("-" to "―")
    private val notKnownIso = mapOf("CTRL" to "⎈", "ALT" to "⎇", "ESC" to "⎋")

    /**
     * 按键的显示文本。
     *
     * 每个样式对应的字形集合逐个对齐 termux `ExtraKeysConstants.EXTRA_KEY_DISPLAY_MAPS`：
     * `default` 只换方向键与最常用的那几个（回车、Tab、退格、抽屉……），`arrows-all` 才
     * 加上 Home/End/PgUp/PgDn 的角标，`all` 再把 ESC/CTRL/ALT 换成 ISO 符号，`none` 一个
     * 都不换。`allCaps` 对应 `extra-keys-text-all-caps`：只影响字母标签，符号与字形不受影响。
     */
    fun displayLabel(name: String, style: ExtraKeysStyle, allCaps: Boolean): String {
        val glyphs =
            when (style) {
                ExtraKeysStyle.DEFAULT -> arrows + wellKnown + nicer
                ExtraKeysStyle.ARROWS_ONLY -> arrows + nicer
                ExtraKeysStyle.ARROWS_ALL -> arrows + wellKnown + lessKnown + nicer
                ExtraKeysStyle.ALL -> arrows + wellKnown + lessKnown + nicer + notKnownIso
                ExtraKeysStyle.NONE -> emptyMap()
            }
        // 别名先归一：上游的字形表查的是**归一后**的名字（`ExtraKeyButton` 先
        // `replaceAlias` 再查显示表），故 `ESCAPE` 与 `ESC` 画同一个字形。
        val normalized = normalizeKeyName(name)
        val glyph = glyphs[normalized]
        if (glyph != null) return glyph
        // 没有字形：显示键名本身。别名改过名字时用归一后的名字（上游亦然：
        // `BACKSLASH` 显示 `\`），没改过时保留用户写的大小写。
        return if (allCaps) normalized else literalText(name, normalized)
    }

    /**
     * 字面量键真正发送的文本：别名改过名字时用归一名（`BACKSLASH` → `\`），
     * 否则保留用户写的大小写（`a` 仍发 `a`，不因键帽全大写而变成 `A`）。
     */
    fun literalText(name: String, normalized: String = normalizeKeyName(name)): String =
        if (normalized == name.uppercase()) name else normalized
}
