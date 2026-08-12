package terminal.emulator.bridge

import com.squareup.moshi.JsonClass
import com.squareup.moshi.Moshi

/**
 * Structured font metadata returned by the native font pipeline.
 *
 * Uses Moshi with codegen (`@JsonClass(generateAdapter = true)`) for
 * R8-safe JSON parsing without runtime reflection. The native side
 * serializes `FontInfo` as JSON via JNI; this adapter deserializes it
 * on the Kotlin side.
 *
 * Replaces manual line-by-line text parsing in `parseFontInfo` for
 * structured data; the legacy text format is kept for backward
 * compatibility with older native builds.
 */
@JsonClass(generateAdapter = true)
data class FontMetadata(
    /** Active font family name (e.g. "Liberation Mono"). */
    val activeFamily: String = "",
    /** Resolved font file path on device. */
    val filePath: String = "",
    /** Font size in design units. */
    val fontSize: Float = 0f,
    /** CJK fallback family name, empty if none. */
    val cjkFallback: String = "",
    /** Nerd Font fallback family name, empty if none. */
    val nerdFallback: String = "",
    /** Emoji fallback family name, empty if none. */
    val emojiFallback: String = "",
    /** Whether the primary font has a CJK character coverage. */
    val primaryIsCjk: Boolean = false,
)

/**
 * Moshi instance configured for this module. Thread-safe (Moshi is
 * immutable after build). Uses KSP-generated adapter (no runtime
 * reflection — R8-safe).
 */
object FontMetadataCodec {
    private val moshi: Moshi = Moshi.Builder().build()

    private val adapter =
        moshi.adapter(FontMetadata::class.java)

    /**
     * Parse a JSON string into [FontMetadata]. Returns `null` on
     * malformed input (never throws — safe for JNI boundary).
     */
    fun fromJson(json: String): FontMetadata? = runCatching { adapter.fromJson(json) }.getOrNull()

    /**
     * Serialize [FontMetadata] to JSON string.
     */
    fun toJson(metadata: FontMetadata): String = adapter.toJson(metadata)
}
