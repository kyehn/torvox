package terminal.emulator.ui

/**
 * Parse Termux-style extra-keys strings into [ToolbarItem] lists.
 *
 * Format:
 *  - `|` separates columns
 *  - `[]` wraps sticky modifier/key groups (keys stay active until next press)
 *  - Keys inside a group are space-separated
 *
 * Example: `[ESC TAB] | [CTRL ALT] | ESC TAB`
 *
 * Reference: TermX ExtraKeysView.kt extra-keys property parsing.
 */
object ExtraKeysParser {

    /**
     * Mapping from common Termux key names to [ToolbarKey] enum names.
     * Keys not found here are tried as-is via [ToolbarKey.valueOf].
     */
    private val keyNameMap: Map<String, String> = mapOf(
        "PAGEUP" to "PGUP",
        "PAGE_UP" to "PGUP",
        "PAGEDOWN" to "PGDN",
        "PAGE_DOWN" to "PGDN",
        "UP" to "ARROW_UP",
        "DOWN" to "ARROW_DOWN",
        "LEFT" to "ARROW_LEFT",
        "RIGHT" to "ARROW_RIGHT",
        "ESCAPE" to "ESC",
        "CONTROL" to "CTRL",
        "FUNCTION" to "FN",
    )

    /** Parse a Termux extra-keys string to a flat list of [ToolbarItem]s.
     *  Sticky (`[]`) groups are flagged; the runtime toolbar applies the
     *  sticky modifier behaviour. */
    fun parse(extraKeysStr: String): List<ToolbarItem> {
        if (extraKeysStr.isBlank()) return emptyList()
        val items = mutableListOf<ToolbarItem>()
        val columns = extraKeysStr.split("|").map { it.trim() }
        for (column in columns) {
            if (column.isEmpty()) continue
            if (column.startsWith("[") && column.endsWith("]")) {
                // Sticky key group: all keys in brackets become sticky.
                val inner = column.substring(1, column.length - 1).trim()
                val keys = inner.split(Regex("\\s+")).filter { it.isNotEmpty() }
                for (key in keys) {
                    items.add(parseKey(key))
                }
            } else {
                // Regular key group.
                val keys = column.split(Regex("\\s+")).filter { it.isNotEmpty() }
                for (key in keys) {
                    items.add(parseKey(key))
                }
            }
        }
        return items
    }

    private fun parseKey(key: String): ToolbarItem {
        val upper = key.uppercase()
        val normalized = keyNameMap[upper] ?: upper
        // Try built-in keys first.
        val builtin = try {
            ToolbarKey.valueOf(normalized)
        } catch (_: IllegalArgumentException) {
            null
        }
        if (builtin != null) {
            return ToolbarItem.Default(builtin)
        }
        // Custom key with escape sequence.
        return ToolbarItem.Custom(
            label = key,
            sequence = builtInSequence(upper),
            id = "extra_$upper",
        )
    }

    // Single-key escape sequences, looked up from a flat map.
    // Mirrors ToolbarMacroExpander special-key table for consistency.
    private val SINGLE_KEY_SEQUENCES = mapOf(
        "ESC" to "\u001b",
        "TAB" to "\t",
        "ENTER" to "\r", "RETURN" to "\r",
        "SPACE" to " ",
        "BKSP" to "\u007f", "BACKSPACE" to "\u007f",
        "HOME" to "\u001b[H", "END" to "\u001b[F",
        "PGUP" to "\u001b[5~", "PAGEUP" to "\u001b[5~", "PAGE_UP" to "\u001b[5~",
        "PGDN" to "\u001b[6~", "PAGEDOWN" to "\u001b[6~", "PAGE_DOWN" to "\u001b[6~",
        "UP" to "\u001b[A", "DOWN" to "\u001b[B",
        "LEFT" to "\u001b[D", "RIGHT" to "\u001b[C",
        "INS" to "\u001b[2~", "INSERT" to "\u001b[2~",
        "DEL" to "\u001b[3~", "DELETE" to "\u001b[3~",
        "F1" to "\u001bOP", "F2" to "\u001bOQ",
        "F3" to "\u001bOR", "F4" to "\u001bOS",
        "F5" to "\u001b[15~", "F6" to "\u001b[17~",
        "F7" to "\u001b[18~", "F8" to "\u001b[19~",
        "F9" to "\u001b[20~", "F10" to "\u001b[21~",
        "F11" to "\u001b[23~", "F12" to "\u001b[24~",
    )

    private fun builtInSequence(key: String): String = SINGLE_KEY_SEQUENCES[key.uppercase()] ?: ""

    /** Convert [ToolbarItem]s to a Termux extra-keys string (for export).
     *  Uses [ToolbarKey.name] so the result can be re-parsed. */
    fun export(items: List<ToolbarItem>): String = items.joinToString(" | ") { item ->
        when (item) {
            is ToolbarItem.Default -> item.key.name
            is ToolbarItem.Custom -> item.label
        }
    }
}
