package terminal.emulator.settings

/**
 * `~/.termux/colors.properties` 的解析结果：对终端主题的**部分覆盖**。
 *
 * 上游 termux 只有一份写死的配色方案（`TerminalColorScheme.DEFAULT_COLORSCHEME`），
 * `colors.properties` 是叠加在它上面的覆盖；本仓有 13 套主题，覆盖同样叠加在当前主题
 * 之上，未写的槽沿用主题原色。
 *
 * 色值一律 `0xRRGGBB`：alpha 恒为不透明（上游的 `TerminalColors.parse` 也只产出不透明色）。
 *
 * 这里只搬运用户**写了的**颜色：光标色随背景亮度的自适应属于调色板规则，由原生的
 * `Palette::with_overrides` 处理（见 `theme.rs`）。Kotlin 侧再实现一份，两端的规则
 * 就有了两份，改一处忘另一处时表现是「改了背景，光标有时跟着变有时不变」。
 */
data class ColorsProperties(
    val foreground: Int? = null,
    val background: Int? = null,
    val cursor: Int? = null,
    /** ANSI 十六色槽 0..15 的覆盖值。 */
    val ansi: Map<Int, Int> = emptyMap(),
) {
    /** 没有覆盖任何槽位：等价于文件不存在。 */
    val isEmpty: Boolean
        get() = foreground == null && background == null && cursor == null && ansi.isEmpty()

    /**
     * 转成原生覆盖通道的 19 槽数组：背景、前景、光标，随后 16 个 ANSI 色。
     *
     * 未覆盖的槽填 [NO_OVERRIDE]，由原生按当前主题补齐——Kotlin 侧不持有主题色副本，
     * 替它猜一个值就是「改了背景，光标跟着变白」。槽位顺序是跨语言契约，与原生
     * `theme::PALETTE_OVERRIDE_SLOTS` 对齐。
     *
     * [NO_OVERRIDE] 取 `-1`：色值是 `0xRRGGBB`（最高 `0xFFFFFF`），负值落在色值域之外，
     * 故它不可能与任何合法颜色相撞。
     */
    fun toPaletteOverride(): IntArray {
        val slots = IntArray(PALETTE_OVERRIDE_SLOTS) { NO_OVERRIDE }
        background?.let { slots[SLOT_BACKGROUND] = it }
        foreground?.let { slots[SLOT_FOREGROUND] = it }
        cursor?.let { slots[SLOT_CURSOR] = it }
        for ((index, color) in ansi) {
            slots[SLOT_ANSI_FIRST + index] = color
        }
        return slots
    }

    companion object {
        /** 覆盖槽位数：背景、前景、光标 + 16 ANSI。与原生 `theme::PALETTE_OVERRIDE_SLOTS` 一致。 */
        const val PALETTE_OVERRIDE_SLOTS = 19

        /** 该槽位不覆盖，由原生取主题原色。与原生 `NO_OVERRIDE` 一致。 */
        const val NO_OVERRIDE = -1

        private const val SLOT_BACKGROUND = 0
        private const val SLOT_FOREGROUND = 1
        private const val SLOT_CURSOR = 2
        private const val SLOT_ANSI_FIRST = 3
    }
}

/** 一次解析的完整结果：覆盖与错误列表。 */
data class ColorsPropertiesLoad(
    val properties: ColorsProperties = ColorsProperties(),
    val errors: List<String> = emptyList(),
)

/**
 * `colors.properties` 的纯解析器：文本 → [ColorsProperties] + 错误列表。
 *
 * 键与色值语法照上游 termux v0.119.0-beta.3（`TerminalColorScheme.updateWith` 与
 * `TerminalColors.parse`）。与上游不同的一处是容错：上游遇到不认识的键直接抛异常，
 * 整份文件都不生效；本仓与 `termux.properties` 同口径——坏键只作废自己，其余照常生效。
 */
internal object ColorsPropertiesParser {
    fun parse(text: String): ColorsPropertiesLoad {
        val errors = mutableListOf<String>()
        var foreground: Int? = null
        var background: Int? = null
        var cursor: Int? = null
        // 上游把 color256/257/258 写进与 foreground/background/cursor 相同的三个下标，
        // 故它们是别名。显式键优先于别名：上游按 Properties 的遍历顺序覆盖，
        // 两种写法并存时结果不确定，本仓不让结果依赖哈希顺序。
        var aliasedForeground: Int? = null
        var aliasedBackground: Int? = null
        var aliasedCursor: Int? = null
        val ansi = mutableMapOf<Int, Int>()

        fun color(key: String, raw: String): Int? {
            val parsed = parseXQueryColor(raw)
            if (parsed == null) errors += "$key: '$raw' 不是 XQueryColor 色值；已忽略"
            return parsed
        }

        // 256 色立方由 libghostty-vt 按 xterm 标准生成，调色板里没有这 256 个槽，
        // 覆盖它们没有去处，只能明确拒绝并说明。
        fun ansiSlot(key: String, raw: String) {
            val slot = key.removePrefix("color").toIntOrNull()
            when {
                slot == null -> errors += "$key: 槽位号不是数字；已忽略"
                slot !in 0..15 -> errors += "$key: 本应用只支持 color0..color15；已忽略"
                else -> color(key, raw)?.let { ansi[slot] = it }
            }
        }

        for ((key, value) in PropertiesFile.parse(text)) {
            when (key) {
                "foreground" -> foreground = color(key, value)
                "background" -> background = color(key, value)
                "cursor" -> cursor = color(key, value)
                "color256" -> aliasedForeground = color(key, value)
                "color257" -> aliasedBackground = color(key, value)
                "color258" -> aliasedCursor = color(key, value)
                else -> if (key.startsWith("color")) ansiSlot(key, value)
            }
        }

        // 显式键优先于别名。
        val effectiveForeground = foreground ?: aliasedForeground
        val effectiveBackground = background ?: aliasedBackground
        // 光标色只搬运用户写了的值：背景被覆盖而光标未指定时，光标随背景亮度的自适应
        // 由原生的调色板合并处理（上游 `TerminalColorScheme.setCursorColorForBackground`
        // 的口径），Kotlin 侧不再算一遍。
        val effectiveCursor = cursor ?: aliasedCursor

        return ColorsPropertiesLoad(
            ColorsProperties(effectiveForeground, effectiveBackground, effectiveCursor, ansi),
            errors,
        )
    }
}

/**
 * XQueryColor 语法的色值解析（上游 `TerminalColors.parse`）：`0xRRGGBB`。
 *
 * 支持 `#RGB`、`#RRGGBB`、`#RRRGGGBBB`、`#RRRRGGGGBBBB` 与 `rgb:R/G/B`；三个分量等宽，
 * 宽度不足 8 位时按上游口径缩放（`分量 * 255 / (2^(4*位宽) - 1)`）。
 */
internal fun parseXQueryColor(raw: String): Int? {
    val text = raw.trim()
    val skipInitial: Int
    val skipBetween: Int
    if (text.startsWith("#")) {
        skipInitial = 1
        skipBetween = 0
    } else if (text.startsWith("rgb:")) {
        skipInitial = 4
        skipBetween = 1
    } else {
        return null
    }
    val colorChars = text.length - skipInitial - 2 * skipBetween
    if (colorChars <= 0 || colorChars % 3 != 0) return null
    val componentLength = colorChars / 3
    // 上游的 XQueryColor 记法每个分量 1..4 位十六进制。
    if (componentLength > 4) return null
    val scale = 255.0 / ((1L shl (componentLength * 4)) - 1)
    val components = (0 until 3).map { index ->
        val start = skipInitial + index * (componentLength + skipBetween)
        text.substring(start, start + componentLength)
    }
    if (components.any { component -> component.any { it.digitToIntOrNull(16) == null } }) return null
    val red = (components[0].toLong(16) * scale).toInt()
    val green = (components[1].toLong(16) * scale).toInt()
    val blue = (components[2].toLong(16) * scale).toInt()
    return (red shl 16) or (green shl 8) or blue
}
