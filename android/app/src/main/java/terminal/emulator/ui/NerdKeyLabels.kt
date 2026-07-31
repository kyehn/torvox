package terminal.emulator.ui

object NerdKeyLabels {
    private val map =
        mapOf(
            "ESC" to "\uEE59",
            "TAB" to "\uEB8A",
            "HOME" to "\uEB90",
            "END" to "\uEB94",
            "PGUP" to "\uEB96",
            "PGDN" to "\uEB95",
            "CTRL" to "CTRL",
            "ALT" to "ALT",
            // U+F040E is outside the BMP and must be written as a
            // surrogate pair; "\uF040E" would truncate to U+F040 + 'E'.
            "SCROLL" to "\uDB81\uDC0E",
        )

    fun label(key: String): String = map[key] ?: key
}
