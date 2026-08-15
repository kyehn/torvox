package terminal.emulator.ui

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.annotation.StringRes
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import terminal.emulator.R
import terminal.emulator.bridge.pollEventJson
import androidx.core.content.edit

enum class ToolbarKey(
    val defaultLabel: String,
    val sequence: String,
    /** Display symbol override (e.g. drawer glyph differs from its label). */
    val symbol: String? = null,
    /** Test-tag override; defaults to "Key_<defaultLabel>". */
    val testTag: String? = null,
    /** Accessibility description resource; defaults to defaultLabel. */
    @StringRes val contentDescriptionRes: Int? = null,
    /** Repeats the key sequence while held (arrow keys). */
    val repeatable: Boolean = false,
    /** Toggle modifier key (CTRL/ALT) driven by ModifierState. */
    val modifier: Boolean = false,
) {
    ESC("ESC", "\u001b", contentDescriptionRes = R.string.escape),
    DRAWER("\u2261", "", symbol = "\u2630", testTag = "Key_DRAWER", contentDescriptionRes = R.string.open_session_drawer),
    SCROLL("SCROLL", "", contentDescriptionRes = R.string.toggle_scroll),
    HOME("HOME", "\u001b[H", contentDescriptionRes = R.string.home_key),
    ARROW_UP("\u2191", "\u001b[A", contentDescriptionRes = R.string.arrow_up, repeatable = true),
    END("END", "\u001b[F", contentDescriptionRes = R.string.end_key),
    PGUP("PGUP", "\u001b[5~", contentDescriptionRes = R.string.page_up),
    TAB("TAB", "\t", contentDescriptionRes = R.string.tab_key),
    CTRL("CTRL", "", contentDescriptionRes = R.string.control_toggle, modifier = true),
    ALT("ALT", "", contentDescriptionRes = R.string.alt_toggle, modifier = true),
    ARROW_LEFT("\u2190", "\u001b[D", contentDescriptionRes = R.string.arrow_left, repeatable = true),
    ARROW_DOWN("\u2193", "\u001b[B", contentDescriptionRes = R.string.arrow_down, repeatable = true),
    ARROW_RIGHT("\u2192", "\u001b[C", contentDescriptionRes = R.string.arrow_right, repeatable = true),
    PGDN("PGDN", "\u001b[6~", contentDescriptionRes = R.string.page_down),
    FN("FN", "", contentDescriptionRes = R.string.function_key_layer),
    COMPOSE("COMPOSE", "", contentDescriptionRes = R.string.compose_key),

    /** termux `KEYBOARD` special key: toggle the soft keyboard visibility. */
    KEYBOARD("KEYBOARD", "", symbol = "\u2328", contentDescriptionRes = R.string.toggle_keyboard),
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
    /** Row weight — wider keys take proportionally more space. */
    abstract val width: Int

    /** Secondary key label shown on long press. */
    abstract val secondaryLabel: String?

    /** Secondary key sequence sent on long press. */
    abstract val secondarySequence: String?

    data class Default(
        val key: ToolbarKey,
        /** Row weight — wider keys take proportionally more space. */
        override val width: Int = 1,
        /** Secondary key label shown on long press. */
        override val secondaryLabel: String? = null,
        /** Secondary key sequence sent on long press. */
        override val secondarySequence: String? = null,
    ) : ToolbarItem() {
        val label: String get() = key.defaultLabel
        val testTag: String get() = "Key_${key.defaultLabel}"
    }

    data class Custom(
        val label: String,
        val sequence: String,
        val id: String = "custom_${System.currentTimeMillis()}",
        /** Space-separated macro, termux extra-keys
         *  semantics); when non-null the sequence field is ignored. */
        val macro: String? = null,
        /** Row weight — wider keys take proportionally more space. */
        override val width: Int = 1,
        /** Secondary key label shown on long press. */
        override val secondaryLabel: String? = null,
        /** Secondary key sequence sent on long press. */
        override val secondarySequence: String? = null,
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

    /** Observe layout changes written by other parts of the app (e.g. the
     *  settings screen) so the modifier bar can recompose live. */
    fun registerLayoutListener(
        listener: (List<ToolbarItem>) -> Unit,
    ): SharedPreferences.OnSharedPreferenceChangeListener {
        val changeListener =
            SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
                if (key == "layout") listener(getLayout())
            }
        sharedPreferences.registerOnSharedPreferenceChangeListener(changeListener)
        return changeListener
    }

    fun unregisterLayoutListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        sharedPreferences.unregisterOnSharedPreferenceChangeListener(listener)
    }

    fun getLayout(): List<ToolbarItem> {
        val json = sharedPreferences.getString("layout", null) ?: return defaultLayout()
        return try {
            pollEventJson.decodeFromString<List<ToolbarItemDto>>(json).map { dto ->
                if (dto.key != null) {
                    ToolbarItem.Default(
                        key = ToolbarKey.valueOf(dto.key),
                        width = dto.width ?: 1,
                        secondaryLabel = dto.secondaryLabel,
                        secondarySequence = dto.secondarySequence,
                    )
                } else {
                    ToolbarItem.Custom(
                        label = dto.label.orEmpty(),
                        sequence = dto.sequence.orEmpty(),
                        id = dto.id ?: "custom_${System.currentTimeMillis()}",
                        macro = dto.macro,
                        width = dto.width ?: 1,
                        secondaryLabel = dto.secondaryLabel,
                        secondarySequence = dto.secondarySequence,
                    )
                }
            }
        } catch (e: Exception) {
            Log.w("ToolbarPreferences", "loadLayout failed", e)
            defaultLayout()
        }
    }

    fun saveLayout(items: List<ToolbarItem>) {
        val dtos =
            items.map { item ->
                when (item) {
                    is ToolbarItem.Default ->
                        ToolbarItemDto(
                            key = item.key.name,
                            width = item.width,
                            secondaryLabel = item.secondaryLabel,
                            secondarySequence = item.secondarySequence,
                        )

                    is ToolbarItem.Custom ->
                        ToolbarItemDto(
                            label = item.label,
                            sequence = item.sequence,
                            id = item.id,
                            macro = item.macro,
                            width = item.width,
                            secondaryLabel = item.secondaryLabel,
                            secondarySequence = item.secondarySequence,
                        )
                }
            }
        sharedPreferences.edit { putString("layout", pollEventJson.encodeToString(dtos)) }
    }

/** JSON shape of a toolbar item in shared preferences.
     *  `key` != null means a [ToolbarItem.Default] (built-in key), otherwise
     *  a [ToolbarItem.Custom] with label/sequence/id. Same on-disk format as
     *  the previous org.json implementation, so existing layouts stay valid. */
    @Serializable
    data class ToolbarItemDto(
        val key: String? = null,
        @SerialName("label") val label: String? = null,
        @SerialName("sequence") val sequence: String? = null,
        @SerialName("id") val id: String? = null,
        @SerialName("macro") val macro: String? = null,
        @SerialName("width") val width: Int? = null,
        @SerialName("secondaryLabel") val secondaryLabel: String? = null,
        @SerialName("secondarySequence") val secondarySequence: String? = null,
    )

    fun defaultLayout(): List<ToolbarItem> = listOf(
        // Mirrors termux-app v0.119.0-beta.3 DEFAULT_IVALUE_EXTRA_KEYS:
        // row1 ESC DRAWER SCROLL HOME UP END PGUP; row2 TAB CTRL ALT LEFT
        // DOWN RIGHT PGDN (ModifierBar splits the flat list 7/7 at the midpoint).
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
