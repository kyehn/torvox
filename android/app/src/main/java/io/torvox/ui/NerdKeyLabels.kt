package io.torvox.ui

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
            "SCROLL" to "\uF0403",
        )

    fun label(key: String): String = map[key] ?: key
}
