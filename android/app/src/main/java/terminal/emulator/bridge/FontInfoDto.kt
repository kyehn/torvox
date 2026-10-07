package terminal.emulator.bridge

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromString
import terminal.emulator.runtime.LogUtil

/** 原生字体管线经 [NativeBridge.getFontInfo] 上报的字体状态（由 Rust `FontInfo` 序列化）。 */
@Serializable
data class FontInfoDto(
    val active: FontActiveDto? = null,
    /** "fallback"（列出回退家族）、"skipped"（主字体已覆盖 CJK）或 "none"。 */
    @SerialName("cjk_state") val cjkState: String = "none",
    @SerialName("cjk_families") val cjkFamilies: List<String> = emptyList(),
    @SerialName("cell_width_px") val cellWidthPx: Float = 0f,
    @SerialName("cell_height_px") val cellHeightPx: Float = 0f,
    /** 逻辑字号，单位 sp（原生 `setFontSizeInPlace` 单位，非 px）。 */
    @SerialName("font_size") val fontSize: Float = 0f,
) {
    val hasRealCjkFallback: Boolean
        get() = cjkState == "fallback" && cjkFamilies.isNotEmpty()

    /** 带真实家族名的 CJK 回退文本，无则返回 null。 */
    fun cjkFallbackText(): String? = cjkFamilies.takeIf { it.isNotEmpty() }?.joinToString(", ")

    companion object {
        /**
         * 解析原生上报的字体状态；失败返回 `null`（面板按「信息不可用」呈现）。
         *
         * 失败必须留痕：Rust 与 Kotlin 的字段一旦漂移，表现只是设置页字体区空白，
         * 没有日志就无法区分「原生尚未初始化」与「契约不一致」。
         */
        fun fromJson(json: String): FontInfoDto? = try {
            pollEventJson.decodeFromString<FontInfoDto>(json)
        } catch (exception: Exception) {
            LogUtil.w("FontInfoDto", "decode failed [${exception.javaClass.simpleName}]: ${json.take(120)}")
            null
        }
    }
}

@Serializable
data class FontActiveDto(val name: String = "", val monospaced: Boolean = false)

/** sp 字号按设备密度换算为像素，仅用于显示（原生管线内部自行按 raster_scale × density 换算）。 */
fun fontSpToPx(fontSizeSp: Float, density: Float): Float = fontSizeSp * density
