package terminal.emulator.settings

/**
 * `~/.termux/termux.properties` 的解析结果。
 *
 * 字段语义与默认值对齐 termux-app `TermuxPropertyConstants`：默认值不是随手取的
 * 常量，而是上游声明的缺省行为，改动其中任何一个都会让同一份用户配置在两个应用里
 * 表现不同。
 *
 * 解析失败一律「保留默认值 + 记录错误」，不让一个写错的键把整份配置作废，更不因
 * 用户文件而崩溃——这是外部配置，不是应用自己的设置数据。
 */
data class TermuxProperties(
    val useFullscreenWorkaround: Boolean = false,
    val fullscreen: Boolean = false,
    val ctrlSpaceWorkaround: Boolean = false,
    /**
     * 启动时不弹软键盘。
     *
     * 默认 `true`，与上游不同（上游默认 `false` = 启动即弹）：本应用启动时从不主动
     * 请求输入法，默认值必须反映既有外观。写 `false` 会在首帧后主动请求一次；平台仍
     * 可能拒绝（窗口尚未取得焦点时 `show` 会被丢弃），此时轻点终端即可。
     */
    val hideSoftKeyboardOnStartup: Boolean = true,
    val extraKeysTextAllCaps: Boolean = true,
    val terminalMarginHorizontalDp: Int = DEFAULT_MARGIN_HORIZONTAL_DP,
    val terminalMarginVerticalDp: Int = DEFAULT_MARGIN_VERTICAL_DP,
    val terminalCursorStyle: TerminalCursorStyle = TerminalCursorStyle.BLOCK,
    /**
     * 回滚行数（`terminal-transcript-rows`）。
     *
     * 区间 100..50000 与上游一致；越界值回退默认（上游 `getDefaultIfNotInRange` 的
     * 口径），不是钳到边界——钳制会让「写成 99999」得到 50000 行回滚，用户要的是
     * 「没配好就用默认」。
     */
    val terminalTranscriptRows: Int = DEFAULT_TRANSCRIPT_ROWS,
    val terminalToolbarHeightScale: Float = 1f,
    val extraKeysStyle: ExtraKeysStyle = ExtraKeysStyle.DEFAULT,
    val extraKeys: List<List<ExtraKey>> = DEFAULT_EXTRA_KEYS,
    val defaultWorkingDirectory: String = "",
    val backKey: BackKeyBehaviour = BackKeyBehaviour.BACK,
    val volumeKeys: VolumeKeysBehaviour = VolumeKeysBehaviour.VIRTUAL,
    val softKeyboardToggleBehaviour: SoftKeyboardToggleBehaviour = SoftKeyboardToggleBehaviour.SHOW_HIDE,
) {
    companion object {
        const val DEFAULT_MARGIN_HORIZONTAL_DP = 3
        const val DEFAULT_MARGIN_VERTICAL_DP = 0

        /** 回滚行数默认值（上游同值）。 */
        const val DEFAULT_TRANSCRIPT_ROWS = 2000

        /** 回滚行数区间（上游同值）。 */
        val TRANSCRIPT_ROWS_RANGE = 100..50000

        /**
         * 默认两行布局：`ESC DRAWER SCROLL HOME ↑ END PGUP` / `TAB CTRL ALT ← ↓ → PGDN`。
         *
         * 与 termux 上游的 `DEFAULT_IVALUE_EXTRA_KEYS` 一致（上游同样含 DRAWER 与
         * SCROLL），只差一处细节：上游把 DRAWER 写成 `{key: 'DRAWER', popup: 'PASTE'}`，
         * 即长按抽屉键粘贴剪贴板。本应用把 DRAWER 做成会话抽屉入口，长按同样是粘贴
         * （见 `ModifierBar` 的 DRAWER 长按动作），语义一致。
         *
         * 默认列数仍守 DESIGN.md 修饰键栏节的「默认 2 行 7 列」。用户一旦写了 `extra-keys`，
         * 布局、列数与键位完全以配置为准（见 [TermuxPropertiesParser]），本默认值不再参与。
         */
        val DEFAULT_EXTRA_KEYS: List<List<ExtraKey>> =
            listOf(
                listOf(
                    ExtraKey("ESC"),
                    ExtraKey("DRAWER", popup = "PASTE"),
                    ExtraKey("SCROLL"),
                    ExtraKey("HOME"),
                    ExtraKey("UP"),
                    ExtraKey("END"),
                    ExtraKey("PGUP"),
                ),
                listOf(
                    ExtraKey("TAB"),
                    ExtraKey("CTRL"),
                    ExtraKey("ALT"),
                    ExtraKey("LEFT"),
                    ExtraKey("DOWN"),
                    ExtraKey("RIGHT"),
                    ExtraKey("PGDN"),
                ),
            )
    }
}

/** 终端光标样式；与原生 `TerminalCursorStyle` 的取值一一对应。 */
enum class TerminalCursorStyle(val nativeValue: Int) {
    BLOCK(0),
    BAR(1),
    UNDERLINE(2),
}

enum class BackKeyBehaviour { BACK, ESCAPE }

enum class VolumeKeysBehaviour { VIRTUAL, VOLUME }

enum class SoftKeyboardToggleBehaviour { SHOW_HIDE, ENABLE_DISABLE }

/**
 * `extra-keys-style`：只决定按键的显示字符，不改变按键本身。
 *
 * 取值与 termux `TerminalToolbarView` 的解析表一致：`default` / `arrows-only` /
 * `arrows-all` / `all` / `none`。字形集合逐项对齐 termux `ExtraKeysConstants`——
 * 同一份配置在两个应用里画出的键栏必须一样，否则「换样式」只是换了个随机外观。
 */
enum class ExtraKeysStyle { DEFAULT, ARROWS_ONLY, ARROWS_ALL, ALL, NONE }

/**
 * `extra-keys` 的一个按键。
 *
 * [name] 是 termux 的键名（`ESC`、`F1`、`UP`、`CTRL`……）或单字符字面量；
 * [popup] 是长按/上档显示的次级字符，本应用把它作为长按发送的文本；
 * [display] 是键帽上显示的文本（`{key: 'X', display: 'Y'}`），为空时按 `extra-keys-style`
 * 取字形；[macro] 是 termux 的宏（`{macro: "CTRL f d"}`），按下时按顺序发一串键
 * （见 `MacroParser` 与 `ModifierBar.buildExtraKeySlots`）。
 */
data class ExtraKey(
    val name: String,
    val popup: String? = null,
    val macro: String? = null,
    val display: String? = null,
)

/** 一份配置的解析结果与错误，供日志与诊断使用。 */
data class TermuxPropertiesLoad(val properties: TermuxProperties, val errors: List<String>)

/**
 * Java Properties 风格的读取器：`key = value` / `key: value`，`#` 与 `!` 起注释，
 * 行尾反斜杠续行，支持 `\n` `\t` `\\` `\ ` `\uXXXX` 转义。
 *
 * 不用 `java.util.Properties`：它会把 `extra-keys` 里的 `:`（对象字面量
 * `{key: '-'}`）当成键值分隔符处理吗？不会——分隔符只在首个未转义的 `=`/`:` 处切一次，
 * 但 `Properties.load` 会吃掉续行前的空白并对 ISO-8859-1 做转码，非 ASCII 键值
 * （中文路径）会被静默破坏。故这里显式按 UTF-8 解析。
 */
internal object PropertiesFile {
    fun parse(text: String): List<Pair<String, String>> {
        val logicalLines = mutableListOf<String>()
        val pending = StringBuilder()
        for (rawLine in text.lineSequence()) {
            val line = if (pending.isEmpty()) rawLine else pending.toString() + rawLine.trimStart()
            if (hasContinuation(line)) {
                pending.clear()
                pending.append(line.dropLast(1))
            } else {
                pending.clear()
                logicalLines += line
            }
        }
        if (pending.isNotEmpty()) logicalLines += pending.toString()

        val entries = mutableListOf<Pair<String, String>>()
        for (line in logicalLines) {
            val trimmed = line.trim()
            if (trimmed.isEmpty() || trimmed.startsWith("#") || trimmed.startsWith("!")) continue
            val separator = firstSeparator(trimmed)
            if (separator < 0) {
                entries += unescape(trimmed) to ""
            } else {
                entries +=
                    unescape(trimmed.substring(0, separator).trim()) to
                    unescape(trimmed.substring(separator + 1).trim())
            }
        }
        return entries
    }

    /** 行尾的反斜杠是续行符；连续两个反斜杠是字面量，不续行。 */
    private fun hasContinuation(line: String): Boolean {
        var backslashes = 0
        var index = line.length - 1
        while (index >= 0 && line[index] == '\\') {
            backslashes++
            index--
        }
        return backslashes % 2 == 1
    }

    private fun firstSeparator(line: String): Int {
        var escaped = false
        for (index in line.indices) {
            val character = line[index]
            if (escaped) {
                escaped = false
                continue
            }
            when (character) {
                '\\' -> escaped = true
                '=', ':' -> return index
            }
        }
        return -1
    }

    private fun unescape(text: String): String {
        if ('\\' !in text) return text
        val output = StringBuilder(text.length)
        var index = 0
        while (index < text.length) {
            val character = text[index]
            if (character != '\\' || index == text.length - 1) {
                output.append(character)
                index++
                continue
            }
            when (val escaped = text[index + 1]) {
                'n' -> output.append('\n')

                't' -> output.append('\t')

                'r' -> output.append('\r')

                'f' -> output.append('\u000C')

                'u' -> {
                    val hex = text.substring(index + 2, minOf(index + 6, text.length))
                    val code = hex.toIntOrNull(16)
                    if (hex.length == 4 && code != null) {
                        output.append(code.toChar())
                        index += 4
                    } else {
                        output.append(escaped)
                    }
                }

                else -> output.append(escaped)
            }
            index += 2
        }
        return output.toString()
    }
}

/**
 * `termux.properties` 的纯解析器：文本 → [TermuxProperties] + 错误列表。
 *
 * 数值越界回退默认并记错（上游 `SharedProperties.getDefaultIfNotInRange` 的口径）：
 * 边距 0..100、回滚行数 100..50000、工具栏高度 0.4..3。
 */
internal object TermuxPropertiesParser {
    fun parse(text: String): TermuxPropertiesLoad {
        val errors = mutableListOf<String>()
        val values = HashMap<String, String>()
        for ((key, value) in PropertiesFile.parse(text)) {
            // 重复键后者生效（与 Properties.load 一致），但必须留痕：
            // 用户同时写两行时通常是在排查为什么改了没效果。
            if (values.put(key, value) != null) {
                errors += "duplicate key '$key'; the last value wins"
            }
        }

        fun boolean(key: String, default: Boolean): Boolean {
            val raw = values[key] ?: return default
            return when (raw.lowercase()) {
                "true" -> true

                "false" -> false

                else -> {
                    errors += "$key: '$raw' is not true/false; keeping $default"
                    default
                }
            }
        }

        fun integer(key: String, default: Int, range: IntRange): Int {
            val raw = values[key] ?: return default
            val parsed = raw.toIntOrNull()
            if (parsed == null) {
                errors += "$key: '$raw' is not an integer; keeping $default"
                return default
            }
            if (parsed !in range) {
                errors += "$key: $parsed out of range $range; keeping $default"
                return default
            }
            return parsed
        }

        fun float(key: String, default: Float, range: ClosedFloatingPointRange<Float>): Float {
            val raw = values[key] ?: return default
            val parsed = raw.toFloatOrNull()
            if (parsed == null || parsed.isNaN()) {
                errors += "$key: '$raw' is not a number; keeping $default"
                return default
            }
            if (parsed !in range) {
                errors += "$key: $parsed out of range $range; keeping $default"
                return default
            }
            return parsed
        }

        val extraKeys =
            values["extra-keys"]?.let { raw ->
                when (val parsed = ExtraKeysParser.parse(raw)) {
                    is ExtraKeysParser.Result.Success -> {
                        errors += parsed.warnings
                        parsed.rows
                    }

                    is ExtraKeysParser.Result.Failure -> {
                        errors += "extra-keys: ${parsed.message}; keeping the default layout"
                        TermuxProperties.DEFAULT_EXTRA_KEYS
                    }
                }
            } ?: TermuxProperties.DEFAULT_EXTRA_KEYS

        val properties =
            TermuxProperties(
                useFullscreenWorkaround = boolean("use-fullscreen-workaround", false),
                fullscreen = boolean("fullscreen", false),
                ctrlSpaceWorkaround = boolean("ctrl-space-workaround", false),
                hideSoftKeyboardOnStartup = boolean("hide-soft-keyboard-on-startup", true),
                extraKeysTextAllCaps = boolean("extra-keys-text-all-caps", true),
                terminalMarginHorizontalDp = integer("terminal-margin-horizontal", 3, 0..100),
                terminalMarginVerticalDp = integer("terminal-margin-vertical", 0, 0..100),
                terminalCursorStyle =
                when ((values["terminal-cursor-style"] ?: "block").lowercase()) {
                    "block" -> TerminalCursorStyle.BLOCK

                    // `beam` 是竖线的另一种叫法（`config.md` 允许的取值）：
                    // 上游只有 `bar`，两者都落成竖线。
                    "bar", "beam" -> TerminalCursorStyle.BAR

                    "underline" -> TerminalCursorStyle.UNDERLINE

                    else -> {
                        errors +=
                            "terminal-cursor-style: '${values["terminal-cursor-style"]}' is unknown; keeping block"
                        TerminalCursorStyle.BLOCK
                    }
                },
                terminalTranscriptRows =
                integer(
                    "terminal-transcript-rows",
                    TermuxProperties.DEFAULT_TRANSCRIPT_ROWS,
                    TermuxProperties.TRANSCRIPT_ROWS_RANGE,
                ),
                terminalToolbarHeightScale = float("terminal-toolbar-height", 1f, 0.4f..3f),
                extraKeysStyle =
                when ((values["extra-keys-style"] ?: "default").lowercase()) {
                    "default" -> ExtraKeysStyle.DEFAULT

                    "arrows-only" -> ExtraKeysStyle.ARROWS_ONLY

                    "arrows-all" -> ExtraKeysStyle.ARROWS_ALL

                    "all" -> ExtraKeysStyle.ALL

                    "none" -> ExtraKeysStyle.NONE

                    else -> {
                        errors += "extra-keys-style: '${values["extra-keys-style"]}' is unknown; using default"
                        ExtraKeysStyle.DEFAULT
                    }
                },
                extraKeys = extraKeys,
                defaultWorkingDirectory = values["default-working-directory"]?.trim().orEmpty(),
                backKey =
                when ((values["back-key"] ?: "back").lowercase()) {
                    "back" -> BackKeyBehaviour.BACK

                    "escape" -> BackKeyBehaviour.ESCAPE

                    else -> {
                        errors += "back-key: '${values["back-key"]}' is unknown; keeping back"
                        BackKeyBehaviour.BACK
                    }
                },
                volumeKeys =
                when ((values["volume-keys"] ?: "virtual").lowercase()) {
                    "virtual" -> VolumeKeysBehaviour.VIRTUAL

                    "volume" -> VolumeKeysBehaviour.VOLUME

                    else -> {
                        errors += "volume-keys: '${values["volume-keys"]}' is unknown; keeping virtual"
                        VolumeKeysBehaviour.VIRTUAL
                    }
                },
                softKeyboardToggleBehaviour =
                when ((values["soft-keyboard-toggle-behaviour"] ?: "show/hide").lowercase()) {
                    "show/hide" -> SoftKeyboardToggleBehaviour.SHOW_HIDE

                    "enable/disable" -> SoftKeyboardToggleBehaviour.ENABLE_DISABLE

                    else -> {
                        errors +=
                            "soft-keyboard-toggle-behaviour: '${values["soft-keyboard-toggle-behaviour"]}' is unknown; using show/hide"
                        SoftKeyboardToggleBehaviour.SHOW_HIDE
                    }
                },
            )
        return TermuxPropertiesLoad(properties, errors)
    }
}

/**
 * `extra-keys` 的迷你解析器。
 *
 * 语法就是 termux 的 JSON 风格字面量：外层数组是行，内层数组是按键；按键可以是
 * 带引号的字符串，或 `{key: 'x', popup: 'y'}`。不引入 JSON 库：这份语法只有三种
 * 记号（数组、对象、字符串），而且 termux 允许单引号与裸键名，标准 JSON 解析器反而
 * 不接收。
 *
 * 本应用不支持的字段（对象形式的 `popup`、只有 `display` 没有按键的对象）MUST 只丢
 * 自己那一项：整份布局因为一个不认识的字段回到默认，等于用户精心配的两行键位被一个
 * 对象整体作废。
 */
internal object ExtraKeysParser {
    sealed interface Result {
        data class Success(
            val rows: List<List<ExtraKey>>,
            /** 保住了布局、但有一项被丢掉时的说明（例如对象形式的 popup）。 */
            val warnings: List<String> = emptyList(),
        ) : Result

        data class Failure(val message: String) : Result
    }

    private class Cursor(private val text: String) {
        var index: Int = 0
        val warnings: MutableList<String> = mutableListOf()

        fun skipWhitespace() {
            while (index < text.length && text[index].isWhitespace()) index++
        }

        fun peek(): Char? {
            skipWhitespace()
            return text.getOrNull(index)
        }

        fun consume(expected: Char) {
            skipWhitespace()
            require(index < text.length && text[index] == expected) {
                "expected '$expected' at offset $index"
            }
            index++
        }

        fun readString(): String {
            skipWhitespace()
            val quote = text.getOrNull(index)
            if (quote != '\'' && quote != '"') {
                val start = index
                while (index < text.length && text[index] !in ",]}") index++
                val bare = text.substring(start, index).trim()
                require(bare.isNotEmpty()) { "empty key at offset $start" }
                return bare
            }
            index++
            val output = StringBuilder()
            while (index < text.length) {
                val character = text[index]
                if (character == '\\' && index + 1 < text.length) {
                    output.append(text[index + 1])
                    index += 2
                    continue
                }
                if (character == quote) {
                    index++
                    return output.toString()
                }
                output.append(character)
                index++
            }
            error("unterminated string starting at offset $index")
        }

        fun readBare(): String {
            skipWhitespace()
            val start = index
            while (index < text.length && (text[index].isLetterOrDigit() || text[index] == '_' || text[index] == '-')) {
                index++
            }
            require(index > start) { "expected a name at offset $start" }
            return text.substring(start, index)
        }

        /** 字段值：字符串或裸名；`{...}`、`[...]` 这类复合值整段跳过并返回 null。 */
        fun readValueOrNull(): String? {
            skipWhitespace()
            return when (text.getOrNull(index)) {
                '{', '[' -> {
                    skipComposite()
                    null
                }

                else -> readString()
            }
        }

        /** 跳过一段配平的 `{...}` / `[...]`；字符串里的括号不参与配对。 */
        fun skipComposite() {
            var depth = 0
            while (index < text.length) {
                when (text[index]) {
                    '\'', '"' -> readString()

                    '{', '[' -> {
                        depth++
                        index++
                    }

                    '}', ']' -> {
                        depth--
                        index++
                        if (depth == 0) return
                    }

                    else -> index++
                }
            }
            error("unterminated object or array")
        }
    }

    fun parse(text: String): Result = try {
        val cursor = Cursor(text)
        cursor.consume('[')
        val rows = mutableListOf<List<ExtraKey>>()
        if (cursor.peek() == ']') {
            cursor.consume(']')
        } else {
            while (true) {
                rows += parseRow(cursor)
                when (cursor.peek()) {
                    ',' -> {
                        cursor.consume(',')
                        // 末行后的逗号允许（手写配置常见）。
                        if (cursor.peek() == ']') break
                    }

                    ']' -> break

                    else -> error("expected ',' or ']' after a row")
                }
            }
            cursor.consume(']')
        }
        cursor.skipWhitespace()
        require(cursor.index == text.length) { "trailing characters after ']'" }
        // 显式空布局（`[]`）就是「不要键栏」，照常返回零行。
        // 但只要写了行、坏项又把这些行全掏空，就按写坏处理退回默认——键栏是抽屉与
        // 滚动锁定的唯一入口，一个空对象不该把它们一起藏起来。
        if (rows.isEmpty()) return Result.Success(emptyList(), cursor.warnings)
        val nonEmptyRows = rows.filter { it.isNotEmpty() }
        require(nonEmptyRows.isNotEmpty()) { "extra-keys contains no keys" }
        Result.Success(nonEmptyRows, cursor.warnings)
    } catch (error: IllegalArgumentException) {
        Result.Failure(error.message ?: "invalid extra-keys")
    } catch (error: IllegalStateException) {
        Result.Failure(error.message ?: "invalid extra-keys")
    }

    private fun parseRow(cursor: Cursor): List<ExtraKey> {
        cursor.consume('[')
        val keys = mutableListOf<ExtraKey>()
        if (cursor.peek() == ']') {
            cursor.consume(']')
            return keys
        }
        while (true) {
            // 单个写坏的对象只丢自己那一项（见类文档）：整行/整份布局作废会让用户
            // 精心配好的键位因为一个不认识的字段全部消失。
            parseKey(cursor)?.let { keys += it }
            when (cursor.peek()) {
                ',' -> {
                    cursor.consume(',')
                    if (cursor.peek() == ']') break
                }

                ']' -> break

                else -> error("expected ',' or ']' inside a row")
            }
        }
        cursor.consume(']')
        return keys
    }

    private fun parseKey(cursor: Cursor): ExtraKey? {
        if (cursor.peek() == '{') {
            cursor.consume('{')
            var key: String? = null
            var popup: String? = null
            var macro: String? = null
            var display: String? = null
            while (true) {
                val name = cursor.readBare()
                cursor.consume(':')
                when (name) {
                    "key" -> key = cursor.readValueOrNull()

                    "popup" -> {
                        popup = cursor.readValueOrNull()
                        if (popup == null) {
                            cursor.warnings +=
                                "extra-keys: object popups are not supported; long-press action dropped"
                        }
                    }

                    "macro" -> macro = cursor.readValueOrNull()

                    "display" -> display = cursor.readValueOrNull()

                    // 未知字段：吃掉值（可能是嵌套对象），别让它拖垮整份布局。
                    else -> cursor.readValueOrNull()
                }
                when (cursor.peek()) {
                    ',' -> cursor.consume(',')
                    '}' -> break
                    else -> error("expected ',' or '}' inside a key object")
                }
            }
            cursor.consume('}')
            if (key == null && macro == null) {
                // `display` 只是键帽文字，没有可执行内容：丢掉这一项，其余按键照常保留。
                cursor.warnings += "extra-keys: object without 'key' or 'macro' dropped (display is only a label)"
                return null
            }
            return ExtraKey(
                name = key ?: display.orEmpty(),
                popup = popup,
                macro = macro,
                display = display,
            )
        }
        return ExtraKey(cursor.readString())
    }
}
