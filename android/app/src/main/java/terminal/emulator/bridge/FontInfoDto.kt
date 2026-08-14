package terminal.emulator.bridge

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString

/**
 * Structured font state reported by the native font pipeline via
 * [NativeBridge.getFontInfo] (JSON serialized from the Rust `FontInfo`).
 * Display formatting happens in the UI layer with string resources.
 */
@Serializable
data class FontInfoDto(
    val active: FontActiveDto? = null,
    /** "fallback" (families listed), "skipped" (primary covers CJK) or "none". */
    @SerialName("cjk_state") val cjkState: String = "none",
    @SerialName("cjk_families") val cjkFamilies: List<String> = emptyList(),
    @SerialName("cell_width_px") val cellWidthPx: Float = 0f,
    @SerialName("cell_height_px") val cellHeightPx: Float = 0f,
    @SerialName("font_size_px") val fontSizePx: Float = 0f,
) {
    val hasRealCjkFallback: Boolean
        get() = cjkState == "fallback" && cjkFamilies.isNotEmpty()

    /** CJK fallback text with real family names, or null when absent. */
    fun cjkFallbackText(): String? = cjkFamilies.takeIf { it.isNotEmpty() }?.joinToString(", ")

    companion object {
        fun fromJson(json: String): FontInfoDto? = try {
            pollEventJson.decodeFromString<FontInfoDto>(json)
        } catch (_: Exception) {
            null
        }

        /** JSON for the "no font loaded yet" placeholder shown before the
         *  renderer reports real data. */
        fun placeholderJson(fontName: String): String = pollEventJson.encodeToString(
            FontInfoDto(active = FontActiveDto(name = fontName, monospaced = false)),
        )
    }
}

@Serializable
data class FontActiveDto(
    val name: String = "",
    val monospaced: Boolean = false,
)
