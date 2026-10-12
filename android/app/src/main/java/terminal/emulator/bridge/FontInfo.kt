package terminal.emulator.bridge

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb

/**
 * 原生字体管线上报的字体状态。
 *
 * 跨 JNI 形状是 [NativeBridge.getFontInfo] 的打包字节（`pack_font_info` 的布局：
 * `u32` 标志 + 三个 `f32` + `u16` 名称数 + NUL 分隔的 UTF-8 名称），Kotlin 侧只做
 * 解包：不在此处留存文本表示，也不再经中间字符串解析。
 */
data class FontInfo(
    val activeName: String,
    val activeMonospaced: Boolean,
    val cjkState: CjkState,
    /** [CjkState.Fallback] 时列出的 CJK 回退族名。 */
    val cjkFamilies: List<String>,
    val cellWidthPx: Float,
    val cellHeightPx: Float,
    /** 逻辑字号，单位 sp（原生 `setFontSizeInPlace` 单位，非 px）。 */
    val fontSizeSp: Float,
) {
    val hasRealCjkFallback: Boolean
        get() = cjkState == CjkState.FALLBACK && cjkFamilies.isNotEmpty()

    /** 带真实家族名的 CJK 回退文本，无则返回 null。 */
    fun cjkFallbackText(): String? = cjkFamilies.takeIf { it.isNotEmpty() }?.joinToString(", ")

    companion object {
        /** 固定部分字节数：标志 4 + 三个 `f32` 12 + 名称数 2。 */
        private const val HEADER_BYTES = 18

        /** 标志位：主字体等宽。 */
        private const val MONOSPACED_FLAG = 1

        /**
         * 解包原生上报的字节。长度不足即抛异常（契约不一致时不静默显示占位数据）。
         */
        fun fromPacked(packed: ByteArray): FontInfo {
            require(packed.size > HEADER_BYTES) { "Packed font info is too short: ${packed.size}" }
            val flags = intAt(packed, 0)
            val nameCount = (packed[16].toInt() and 0xFF) or ((packed[17].toInt() and 0xFF) shl 8)
            // 原生每个名称后必跟一个 NUL，故切分结果恒为「名称数 + 1 个尾部空串」；
            // 少于此即载荷被截断，多于此即尾部有多余字节。
            val names = packed.decodeToString(HEADER_BYTES, packed.size).split('\u0000')
            require(names.size == nameCount + 1) {
                "Packed font info declares $nameCount names but carries ${names.size - 1}"
            }
            return FontInfo(
                activeName = names[0],
                activeMonospaced = flags and MONOSPACED_FLAG != 0,
                cjkState = CjkState.entries[flags ushr 1],
                cjkFamilies = names.drop(1).take(nameCount - 1),
                cellWidthPx = floatAt(packed, 4),
                cellHeightPx = floatAt(packed, 8),
                fontSizeSp = floatAt(packed, 12),
            )
        }

        private fun intAt(packed: ByteArray, offset: Int): Int = (packed[offset].toInt() and 0xFF) or
            ((packed[offset + 1].toInt() and 0xFF) shl 8) or
            ((packed[offset + 2].toInt() and 0xFF) shl 16) or
            ((packed[offset + 3].toInt() and 0xFF) shl 24)

        private fun floatAt(packed: ByteArray, offset: Int): Float = Float.fromBits(intAt(packed, offset))
    }
}

/**
 * CJK 回退状态，判别序号与原生 `CjkFallbackState` 一一对应。
 */
enum class CjkState {
    /** 未解析到任何 CJK 回退族。 */
    NONE,

    /** 主字体不含 CJK，已列出回退族名。 */
    FALLBACK,

    /** 主字体自身已覆盖 CJK，无需回退。 */
    SKIPPED,
}

/**
 * sp 字号换算为设备像素，仅用于显示。
 *
 * @param spToPxScale sp→像素的完整系数（`TerminalRuntime.spToPxScale`：显示密度 ×
 *   系统字体缩放）。字形的实际光栅尺度正是 `sp * 该系数`（原生 `setRasterScale`），
 *   故展示值必须用同一个系数——只乘显示密度会在系统「字体大小」> 1 时报出
 *   小于真实渲染的像素值。
 */
fun fontSpToPx(fontSizeSp: Float, spToPxScale: Float): Float = fontSizeSp * spToPxScale

/** ARGB 整数的 alpha 固定为不透明（主题与字体信息均为不透明色）。 */
internal fun opaqueArgb(color: Color): Int = color.toArgb() or (0xFF shl 24)
