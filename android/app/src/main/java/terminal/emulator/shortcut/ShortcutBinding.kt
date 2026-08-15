package terminal.emulator.shortcut

import android.view.KeyEvent

/**
 * A hardware-keyboard shortcut binding: a primary key plus optional
 * modifier flags.
 *
 * Serialization format is the reterminal pattern — modifier names joined
 * by '|' followed by the key code, e.g. "CTRL|SHIFT|54" for
 * Ctrl+Shift+V (54 = KEYCODE_V). Empty serialization ("" — or a plain
 * key code with no modifiers) means "no shortcut bound".
 *
 * Mirrors reterminal ShortcutBinding.kt:10-114 (research-mid-repos-b.md
 * section 4.2): serialize/deserialize for SharedPreferences/DataStore
 * storage, matches() for dispatch, and key-code guards so system keys
 * (HOME/BACK/POWER/volume) or bare modifier keys can never be bound.
 */
data class ShortcutBinding(
    val key: Int,
    val ctrl: Boolean = false,
    val shift: Boolean = false,
    val alt: Boolean = false,
    val meta: Boolean = false,
) {
    /**
     * True when nothing is bound. A binding with only modifier flags and
     * no key is meaningless, so it deserializes to empty.
     */
    fun isEmpty(): Boolean = key < 0

    /**
     * Serializes as the reterminal bitfield-style string: modifier names
     * (uppercase, '|'-separated) followed by the key code, e.g.
     * "CTRL|SHIFT|54". A binding without modifiers serializes to just the
     * key code. An empty binding serializes to "".
     */
    fun serialize(): String {
        if (isEmpty()) return ""
        val mods =
            buildList {
                if (ctrl) add("CTRL")
                if (shift) add("SHIFT")
                if (alt) add("ALT")
                if (meta) add("META")
            }
        return if (mods.isEmpty()) "$key" else "${mods.joinToString("|")}|$key"
    }

    /**
     * True when [event] is the ACTION_DOWN of exactly this binding.
     *
     * Modifiers are compared strictly (all four flags must equal the
     * event's pressed state) so an unbound Ctrl combination is never
     * swallowed and always reaches the terminal (e.g. Ctrl+C keeps
     * generating SIGINT). Key events that repeat are also ignored:
     * dispatching an action on every auto-repeat would e.g. create
     * dozens of sessions while the user holds the chord down.
     */
    fun matches(event: KeyEvent): Boolean {
        if (isEmpty() || event.action != KeyEvent.ACTION_DOWN || event.repeatCount > 0) {
            return false
        }
        if (event.keyCode != key) return false
        return modifierFlagsMatch(event.metaState, ctrl, shift, alt, meta)
    }

    /**
     * Human-readable label for the settings UI, e.g. "Ctrl+Shift+V".
     */
    fun toDisplayString(): String {
        if (isEmpty()) return "—"
        val parts =
            buildList {
                if (ctrl) add("Ctrl")
                if (shift) add("Shift")
                if (alt) add("Alt")
                if (meta) add("Meta")
                add(displayKeyName(key))
            }
        return parts.joinToString("+")
    }

    companion object {
        /** Sentinel for "no binding" — never a real Android key code. */
        const val EMPTY_KEY = -1

        val EMPTY = ShortcutBinding(EMPTY_KEY)

        /**
         * Key codes that must never be bound: system/home navigation,
         * power/volume hardware keys, and the modifier keys themselves.
         * Rejected at capture time (ShortcutCaptureDialog) and at
         * deserialize time so a corrupt persisted value cannot hijack
         * system buttons (reterminal ShortcutBinding.kt RESERVED_KEY_CODES,
         * research-mid-repos-b.md 4.2).
         */
        val RESERVED_KEY_CODES: Set<Int> =
            setOf(
                KeyEvent.KEYCODE_HOME,
                KeyEvent.KEYCODE_BACK,
                KeyEvent.KEYCODE_APP_SWITCH,
                KeyEvent.KEYCODE_POWER,
                KeyEvent.KEYCODE_CAMERA,
                KeyEvent.KEYCODE_VOLUME_UP,
                KeyEvent.KEYCODE_VOLUME_DOWN,
                KeyEvent.KEYCODE_VOLUME_MUTE,
                KeyEvent.KEYCODE_MENU,
                KeyEvent.KEYCODE_SEARCH,
                KeyEvent.KEYCODE_ASSIST,
                KeyEvent.KEYCODE_EXPLORER,
                KeyEvent.KEYCODE_CTRL_LEFT,
                KeyEvent.KEYCODE_CTRL_RIGHT,
                KeyEvent.KEYCODE_SHIFT_LEFT,
                KeyEvent.KEYCODE_SHIFT_RIGHT,
                KeyEvent.KEYCODE_ALT_LEFT,
                KeyEvent.KEYCODE_ALT_RIGHT,
                KeyEvent.KEYCODE_META_LEFT,
                KeyEvent.KEYCODE_META_RIGHT,
            )

        /**
         * Parses a persisted string back into a binding. Tolerant: any
         * malformed segment or reserved key code yields [EMPTY] instead
         * of throwing, so a corrupt setting can never crash the app or
         * bind a system key.
         */
        fun deserialize(raw: String): ShortcutBinding {
            val trimmed = raw.trim()
            if (trimmed.isEmpty()) return EMPTY
            val parts = trimmed.split('|').map { it.trim().uppercase() }
            if (parts.isEmpty()) return EMPTY
            val keyCode = parts.last().toIntOrNull() ?: return EMPTY
            if (keyCode in RESERVED_KEY_CODES || keyCode < 0) return EMPTY
            val flags =
                parts.dropLast(1).fold(ShortcutBinding(EMPTY_KEY)) { acc, part ->
                    when (part) {
                        "CTRL" -> acc.copy(ctrl = true)
                        "SHIFT" -> acc.copy(shift = true)
                        "ALT" -> acc.copy(alt = true)
                        "META" -> acc.copy(meta = true)
                        else -> return EMPTY // unknown modifier — corrupt
                    }
                }
            return flags.copy(key = keyCode)
        }

        /**
         * Key-name table for the settings display. Falls back to the
         * Android key code for codes we do not name.  Map-based to keep
         * detekt CyclomaticComplexMethod under the threshold.
         */
        private val KEY_NAME_MAP: Map<Int, String> = mapOf(
            KeyEvent.KEYCODE_ENTER to "Enter",
            KeyEvent.KEYCODE_TAB to "Tab",
            KeyEvent.KEYCODE_SPACE to "Space",
            KeyEvent.KEYCODE_DEL to "Backspace",
            KeyEvent.KEYCODE_FORWARD_DEL to "Delete",
            KeyEvent.KEYCODE_ESCAPE to "Esc",
            KeyEvent.KEYCODE_DPAD_UP to "↑",
            KeyEvent.KEYCODE_DPAD_DOWN to "↓",
            KeyEvent.KEYCODE_DPAD_LEFT to "←",
            KeyEvent.KEYCODE_DPAD_RIGHT to "→",
            KeyEvent.KEYCODE_MOVE_HOME to "Home",
            KeyEvent.KEYCODE_MOVE_END to "End",
            KeyEvent.KEYCODE_PAGE_UP to "PgUp",
            KeyEvent.KEYCODE_PAGE_DOWN to "PgDn",
            KeyEvent.KEYCODE_INSERT to "Insert",
            KeyEvent.KEYCODE_GRAVE to "`",
            KeyEvent.KEYCODE_MINUS to "-",
            KeyEvent.KEYCODE_EQUALS to "=",
            KeyEvent.KEYCODE_LEFT_BRACKET to "[",
            KeyEvent.KEYCODE_RIGHT_BRACKET to "]",
            KeyEvent.KEYCODE_BACKSLASH to "\\",
            KeyEvent.KEYCODE_SEMICOLON to ";",
            KeyEvent.KEYCODE_APOSTROPHE to "'",
            KeyEvent.KEYCODE_COMMA to ",",
            KeyEvent.KEYCODE_PERIOD to ".",
            KeyEvent.KEYCODE_SLASH to "/",
        )

        private fun displayKeyName(keyCode: Int): String = KEY_NAME_MAP[keyCode] ?: keyCodeLabel(keyCode)

        private fun keyCodeLabel(keyCode: Int): String {
            val c = codeToChar(keyCode) ?: return "Key$keyCode"
            return c.toString().uppercase()
        }

        /** Printable ASCII for the letter/digit key codes. */
        private fun codeToChar(keyCode: Int): Char? = when (keyCode) {
            in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z -> ('A'.code + (keyCode - KeyEvent.KEYCODE_A)).toChar()
            in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> ('0'.code + (keyCode - KeyEvent.KEYCODE_0)).toChar()
            else -> null
        }
    }
}

/**
 * Pure modifier comparison against a raw key-event metaState bitmask. Uses
 * the same masks the Android framework reads in isCtrlPressed /
 * isShiftPressed / isAltPressed / isMetaPressed, but takes the bitmask
 * directly so `matches`/`dispatch` are unit-testable without
 * instrumentation.
 */
internal fun modifierFlagsMatch(metaState: Int, ctrl: Boolean, shift: Boolean, alt: Boolean, meta: Boolean): Boolean {
    val ctrlPressed = metaState and KeyEvent.META_CTRL_MASK != 0
    val shiftPressed = metaState and KeyEvent.META_SHIFT_MASK != 0
    val altPressed = metaState and KeyEvent.META_ALT_MASK != 0
    val metaPressed = metaState and KeyEvent.META_META_MASK != 0
    return ctrlPressed == ctrl &&
        shiftPressed == shift &&
        altPressed == alt &&
        metaPressed == meta
}
