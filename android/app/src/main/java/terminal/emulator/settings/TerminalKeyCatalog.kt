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

    fun keyCodeFor(name: String): Int? = keyCodes[name.uppercase()]

    fun specialFor(name: String): Special? = specials[name.uppercase()]

    fun isRepetitive(name: String): Boolean = name.uppercase() in repetitive

    fun isKnown(name: String): Boolean = keyCodeFor(name) != null || specialFor(name) != null || name.length == 1

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
     * `allCaps` 对应 `extra-keys-text-all-caps`：只影响字母标签，符号与字形不受影响。
     */
    fun displayLabel(name: String, style: ExtraKeysStyle, allCaps: Boolean): String {
        val glyphs =
            when (style) {
                ExtraKeysStyle.DEFAULT -> arrows + wellKnown + lessKnown + nicer
                ExtraKeysStyle.ARROWS_ONLY -> arrows + nicer
                ExtraKeysStyle.ARROWS_ALL -> arrows + wellKnown + lessKnown + nicer
                ExtraKeysStyle.NO_ARROWS -> wellKnown + lessKnown + nicer
                ExtraKeysStyle.FULL -> arrows + wellKnown + lessKnown + nicer + notKnownIso
            }
        val upper = name.uppercase()
        val glyph = glyphs[upper] ?: glyphs[name]
        if (glyph != null) return glyph
        return if (allCaps) upper else name
    }
}
