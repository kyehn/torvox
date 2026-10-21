package terminal.emulator.bridge

/**
 * 原生字体管线上报的字体状态。
 *
 * 跨 JNI 形状是 [NativeBridge.getFontInfo] 的打包字节（`pack_font_info` 的布局：
 * `u32` 标志 + 三个 `f32` + `u16` 名称数 + `u16` CJK 名称数 + NUL 分隔的 UTF-8
 * 名称），Kotlin 侧只做解包：不在此处留存文本表示，也不再经中间字符串解析。
 */
data class FontInfo(
    val activeName: String,
    val activeMonospaced: Boolean,
    val cjkState: CjkState,
    /** [CjkState.FALLBACK] 时列出的 CJK 回退族名。 */
    val cjkFamilies: List<String>,
    /** 除主字体与 [cjkFamilies] 外实际供出过字形的族名（符号族、区域族等）。 */
    val otherFamilies: List<String>,
    val cellWidthPx: Float,
    val cellHeightPx: Float,
    /** 逻辑字号，单位 sp（原生 `setFontSizeInPlace` 单位，非 px）。 */
    val fontSizeSp: Float,
) {
    val hasRealCjkFallback: Boolean
        get() = cjkState == CjkState.FALLBACK && cjkFamilies.isNotEmpty()

    /**
     * 回退层是否**按设计**缺席（跳过而非缺失）。
     *
     * 这三种跳过都不是问题：主字体已覆盖代表字符、系统语言不是 CJK、
     * 系统语言未知。设置页对它们不得报「未检测到 CJK 备用字体」。
     */
    val isCjkFallbackSkipped: Boolean
        get() = cjkState == CjkState.SKIPPED_PRIMARY_COVERS ||
            cjkState == CjkState.SKIPPED_NON_CJK_LOCALE ||
            cjkState == CjkState.SKIPPED_LOCALE_UNKNOWN

    /**
     * 是否**确实缺失** CJK 回退：原生已上报，且状态既不是「有回退」也不是「按设计跳过」。
     *
     * 只有这一种情况才该报「未检测到 CJK 备用字体」；[CjkState.NOT_REPORTED]
     * 是原生尚未上报，什么都无从判断，不得当成缺失。
     */
    val isCjkFallbackMissing: Boolean
        get() = cjkState == CjkState.NONE

    /** 带真实家族名的 CJK 回退文本，无则返回 null。 */
    fun cjkFallbackText(): String? = cjkFamilies.takeIf { it.isNotEmpty() }?.joinToString(", ")

    companion object {
        /** 固定部分字节数：标志 4 + 三个 `f32` 12 + 名称数 2 + CJK 名称数 2。 */
        private const val HEADER_BYTES = 20

        /** 名称数在头部里的起始偏移。 */
        private const val NAME_COUNT_OFFSET = 16

        /** 标志位：主字体等宽。 */
        private const val MONOSPACED_FLAG = 1

        /**
         * 原生会上报的状态，按判别序号排列。
         *
         * 序号必须与原生 `CjkFallbackState` 一一对应，故本表只列原生那几项；
         * Kotlin 侧的 [CjkState.NOT_REPORTED] 不在其中——它由「原生还没上报」
         * 这一事实构造，不是从字节流里解出来的。
         */
        private val NATIVE_REPORTED_STATES = listOf(
            CjkState.NONE,
            CjkState.FALLBACK,
            CjkState.SKIPPED_PRIMARY_COVERS,
            CjkState.SKIPPED_NON_CJK_LOCALE,
            CjkState.SKIPPED_LOCALE_UNKNOWN,
        )

        /**
         * 解包原生上报的字节。长度不足即抛异常（契约不一致时不静默显示占位数据）。
         */
        fun fromPacked(packed: ByteArray): FontInfo {
            require(packed.size > HEADER_BYTES) { "Packed font info is too short: ${packed.size}" }
            val flags = intAt(packed, 0)
            val nameCount = shortAt(packed, NAME_COUNT_OFFSET)
            val cjkCount = shortAt(packed, NAME_COUNT_OFFSET + 2)
            // 原生每个名称后必跟一个 NUL，故切分结果恒为「名称数 + 1 个尾部空串」；
            // 少于此即载荷被截断，多于此即尾部有多余字节。
            val names = packed.decodeToString(HEADER_BYTES, packed.size).split('\u0000')
            require(names.size == nameCount + 1) {
                "Packed font info declares $nameCount names but carries ${names.size - 1}"
            }
            // 主字体占一个名字，CJK 段紧随其后，其余即「其他被使用字体」。
            require(cjkCount <= nameCount - 1) {
                "Packed font info declares $cjkCount CJK names out of $nameCount"
            }
            // CJK 状态占 bit1 起。序号越界说明原生与 Kotlin 的状态枚举对不上，
            // 契约已经不一致——明确报错，别让下标越界抛一句无来由的异常。
            // NOT_REPORTED 只在 Kotlin 侧存在，原生从不产出，故不接字节流。
            val cjkStateOrdinal = flags ushr 1
            require(cjkStateOrdinal in NATIVE_REPORTED_STATES.indices) {
                "Packed font info declares unknown CJK state $cjkStateOrdinal"
            }
            return FontInfo(
                activeName = names[0],
                activeMonospaced = flags and MONOSPACED_FLAG != 0,
                cjkState = NATIVE_REPORTED_STATES[cjkStateOrdinal],
                cjkFamilies = names.subList(1, cjkCount + 1),
                otherFamilies = names.subList(cjkCount + 1, nameCount),
                cellWidthPx = floatAt(packed, 4),
                cellHeightPx = floatAt(packed, 8),
                fontSizeSp = floatAt(packed, 12),
            )
        }

        private fun shortAt(packed: ByteArray, offset: Int): Int = (packed[offset].toInt() and 0xFF) or
            ((packed[offset + 1].toInt() and 0xFF) shl 8)

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
    /** CJK 语言环境、主字体不覆盖该语言的代表字符，且未解析到任何 CJK 回退族。 */
    NONE,

    /** 主字体不含 CJK，已列出回退族名。 */
    FALLBACK,

    /** 主字体已覆盖本 locale 的 CJK 代表字符：按设计不装语言回退族。 */
    SKIPPED_PRIMARY_COVERS,

    /** 系统语言不是 CJK：按平台语义不装 `lang` 语言回退族。 */
    SKIPPED_NON_CJK_LOCALE,

    /** 系统语言尚未知：无从判断是否需要语言回退族。 */
    SKIPPED_LOCALE_UNKNOWN,

    /**
     * 原生尚未上报字体状态：既不知道有没有回退，也不知道为何没有。
     *
     * 只在原生给出度量之前存在（设置页先渲染、管道后建好）。
     * 与原生判别序号一一对应的约定不适用于本项：原生从不产出它，
     * 故放在末尾以免挪动前五项的序号。它与 `NONE` 的区别正是不把
     * 「还没上报」误报成「确实缺失」。
     */
    NOT_REPORTED,
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
