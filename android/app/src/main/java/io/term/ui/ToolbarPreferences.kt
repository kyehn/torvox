package io.term.ui

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

enum class ToolbarKey(
    val defaultLabel: String,
    val sequence: String,
) {
    ESC("ESC", "\u001b"),
    DRAWER("\u2261", ""),
    SCROLL("SCROLL", ""),
    HOME("HOME", "\u001b[H"),
    ARROW_UP("\u2191", "\u001b[A"),
    END("END", "\u001b[F"),
    PGUP("PGUP", "\u001b[5~"),
    TAB("TAB", "\t"),
    CTRL("CTRL", ""),
    ALT("ALT", ""),
    ARROW_LEFT("\u2190", "\u001b[D"),
    ARROW_DOWN("\u2193", "\u001b[B"),
    ARROW_RIGHT("\u2192", "\u001b[C"),
    PGDN("PGDN", "\u001b[6~"),
    FN("FN", ""),
    PIPE("|", "|"),
    SLASH("/", "/"),
    DASH("-", "-"),
    UNDERSCORE("_", "_"),
    DOT(".", "."),
    EQUALS("=", "="),
    HASH("#", "#"),
    AT("@", "@"),
    AMPERSAND("&", "&"),
    TILDE("~", "~"),
    BACKTICK("`", "`"),
    BANG("!", "!"),
    QUESTION("?", "?"),
}

sealed class ToolbarItem {
    data class Default(
        val key: ToolbarKey,
    ) : ToolbarItem() {
        val label: String get() = key.defaultLabel
        val testTag: String get() = "Key_${key.defaultLabel}"
    }

    data class Custom(
        val label: String,
        val sequence: String,
        val id: String = "custom_${System.currentTimeMillis()}",
    ) : ToolbarItem() {
        val testTag: String get() = "Key_$id"
    }
}

class ToolbarPreferences(
    context: Context,
) {
    private val sharedPreferences: SharedPreferences by lazy {
        context.getSharedPreferences("toolbar_prefs", Context.MODE_PRIVATE)
    }

    fun getLayout(): List<ToolbarItem> {
        val json = sharedPreferences.getString("layout", null) ?: return defaultLayout()
        return try {
            val arr = JSONArray(json)
            (0 until arr.length()).map { i ->
                val obj = arr.getJSONObject(i)
                if (obj.has("key")) {
                    val keyName = obj.getString("key")
                    val key = ToolbarKey.valueOf(keyName)
                    ToolbarItem.Default(key)
                } else {
                    ToolbarItem.Custom(
                        label = obj.getString("label"),
                        sequence = obj.getString("sequence"),
                        id = obj.optString("id", "custom_${System.currentTimeMillis()}"),
                    )
                }
            }
        } catch (e: Exception) {
            Log.w("ToolbarPreferences", "loadLayout failed", e)
            defaultLayout()
        }
    }

    fun saveLayout(items: List<ToolbarItem>) {
        val arr = JSONArray()
        for (item in items) {
            val obj = JSONObject()
            when (item) {
                is ToolbarItem.Default -> {
                    obj.put("key", item.key.name)
                }

                is ToolbarItem.Custom -> {
                    obj.put("label", item.label)
                    obj.put("sequence", item.sequence)
                    obj.put("id", item.id)
                }
            }
            arr.put(obj)
        }
        sharedPreferences.edit().putString("layout", arr.toString()).apply()
    }

    private fun defaultLayout(): List<ToolbarItem> = listOf(
        ToolbarItem.Default(ToolbarKey.ESC),
        ToolbarItem.Default(ToolbarKey.DRAWER),
        ToolbarItem.Default(ToolbarKey.SCROLL),
        ToolbarItem.Default(ToolbarKey.HOME),
        ToolbarItem.Default(ToolbarKey.ARROW_UP),
        ToolbarItem.Default(ToolbarKey.END),
        ToolbarItem.Default(ToolbarKey.PGUP),
        ToolbarItem.Default(ToolbarKey.TAB),
        ToolbarItem.Default(ToolbarKey.CTRL),
        ToolbarItem.Default(ToolbarKey.ALT),
        ToolbarItem.Default(ToolbarKey.ARROW_LEFT),
        ToolbarItem.Default(ToolbarKey.ARROW_DOWN),
        ToolbarItem.Default(ToolbarKey.ARROW_RIGHT),
        ToolbarItem.Default(ToolbarKey.PGDN),
    )
}
