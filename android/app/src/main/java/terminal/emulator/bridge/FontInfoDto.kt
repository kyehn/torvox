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

/**
 * sp 字号换算为设备像素，仅用于显示。
 *
 * @param spToPxScale sp→像素的完整系数（`TerminalRuntime.spToPxScale`：显示密度 ×
 *   系统字体缩放）。字形的实际光栅尺度正是 `sp * 该系数`（原生 `setRasterScale`），
 *   故展示值必须用同一个系数——只乘显示密度会在系统「字体大小」> 1 时报出
 *   小于真实渲染的像素值。
 */
fun fontSpToPx(fontSizeSp: Float, spToPxScale: Float): Float = fontSizeSp * spToPxScale
