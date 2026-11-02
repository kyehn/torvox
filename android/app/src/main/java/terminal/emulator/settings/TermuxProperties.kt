package terminal.emulator.settings

import java.io.StringReader
import java.util.Properties

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
     * 启动时是否隐藏软键盘。
     *
     * 默认 `false`（上游 `TermuxPropertyConstants` 把该键列在「默认 false」的布尔集合里）：
     * 冷启动即弹键盘，与 `DESIGN.md` 的「启动后自动弹出输入法」一致。写 `true` 则启动时
     * 不请求输入法，直到用户轻点终端。
     */
    val hideSoftKeyboardOnStartup: Boolean = false,
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
    /** `shortcut.*` 的 Ctrl 组合键：码点 → 动作；空表示没有配置。 */
    val sessionShortcuts: Map<SessionShortcut, Int> = emptyMap(),
) {
    companion object {
        const val DEFAULT_MARGIN_HORIZONTAL_DP = 3
        const val DEFAULT_MARGIN_VERTICAL_DP = 0

        /** 回滚行数默认值（上游同值）。 */
        const val DEFAULT_TRANSCRIPT_ROWS = 2000

        /** 回滚行数区间（上游同值）。 */
        val TRANSCRIPT_ROWS_RANGE = 100..50000

        /** 终端边距区间（dp，上游同值）。 */
        val MARGIN_RANGE = 0..100

        /** 工具栏高度缩放区间（上游同值）。 */
        val TOOLBAR_HEIGHT_SCALE_RANGE = 0.4f..3f

        /**
         * 默认两行布局：`ESC DRAWER SCROLL HOME ↑ END PGUP` / `TAB CTRL ALT ← ↓ → PGDN`。
         *
         * 与 termux 上游的 `DEFAULT_IVALUE_EXTRA_KEYS` 一致（上游同样含 DRAWER 与
         * SCROLL），只差一处细节：上游把 DRAWER 写成 `{key: 'DRAWER', popup: 'PASTE'}`，
         * 即长按抽屉键粘贴剪贴板。本应用把 DRAWER 做成会话抽屉入口，长按同样是粘贴
         * （popup 是完整的按键，见 [ExtraKey]），语义一致。
         *
         * 默认列数仍守 DESIGN.md 修饰键栏节的「默认 2 行 7 列」。用户一旦写了 `extra-keys`，
         * 布局、列数与键位完全以配置为准（见 [TermuxPropertiesParser]），本默认值不再参与。
         */
        val DEFAULT_EXTRA_KEYS: List<List<ExtraKey>> =
            listOf(
                listOf(
                    ExtraKey("ESC"),
                    ExtraKey("DRAWER", popup = ExtraKey("PASTE")),
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
 * [popup] 是长按触发的次级按键——它本身就是一个按键，故与主键同构：可以是键名、
 * 字面量，也可以是宏（上游 termux 的 `popup` 就是完整的按键，见
 * `ExtraKeysInfo.normalizeKeyConfig`）；
 * [display] 是键帽上显示的文本（`{key: 'X', display: 'Y'}`），为空时按 `extra-keys-style`
 * 取字形；[macro] 是 termux 的宏（`{macro: "CTRL f d"}`），按下时按顺序发一串键
 * （见 `MacroParser` 与 `ModifierBar.buildExtraKeySlots`）。
 */
data class ExtraKey(
    val name: String,
    val popup: ExtraKey? = null,
    val macro: String? = null,
    val display: String? = null,
)

/** 一份配置的解析结果与错误，供日志与诊断使用。 */
data class TermuxPropertiesLoad(val properties: TermuxProperties, val errors: List<String>)

/**
 * 会话快捷键（`shortcut.*`），值是一段 `Ctrl+<字符>` 的码点。
 *
 * 上游把配置存在 `MAP_SESSION_SHORTCUTS`，由 `TermuxTerminalViewClient` 在 Ctrl
 * 组合键上匹配。这里只保留 `CONFIG.md` 声明的三项——上游还有
 * `shortcut.rename-session`，但本应用没有重命名 UI（会话标题不可改），它与其他
 * 未声明的键一样被跳过。
 */
enum class SessionShortcut(val propertyKey: String) {
    CREATE("shortcut.create-session"),
    NEXT("shortcut.next-session"),
    PREVIOUS("shortcut.previous-session"),
}

/** 上游支持、本应用不支持的会话快捷键：跳过并报告。 */
private const val UNSUPPORTED_RENAME_SHORTCUT = "shortcut.rename-session"

/**
 * Java Properties 风格的读取器：`key = value` / `key: value` / `key value`，
 * `#` 与 `!` 起注释，行尾反斜杠续行，支持 `\n` `\t` `\\` `\ ` `\uXXXX` 转义。
 *
 * 语法交给 JDK 的 [Properties]：它是该格式的参考实现，分隔符、续行与转义规则都随
 * JDK 维护，本仓不再自研一份。用 [StringReader] 而非 `load(InputStream)`——后者按
 * ISO-8859-1 解码，会把非 ASCII 键值（中文路径）静默破坏。
 *
 * JDK 对非法 `\u` 转义抛异常并拒绝整份文件，而本仓的口径是「写坏一个键只废掉那个
 * 键」：故先把这类转义降级成字面量（`\u12` → `u12`），其余交给 JDK 解析。
 */
internal object PropertiesFile {
    fun parse(text: String): List<Pair<String, String>> {
        val properties = RecordingProperties()
        properties.load(StringReader(downgradeMalformedUnicodeEscapes(text)))
        return properties.recorded
    }
}

/**
 * 非法 `\u` 转义（后随不足四位十六进制）：降级成字面量，不让 JDK 拒绝整份文件。
 *
 * 只认「引出转义的那一个反斜杠」：JDK 按两两配对消费反斜杠，`\\` 是字面反斜杠，其后的
 * `u` 只是普通字符。故连续 N 个反斜杠后接 `u` 时，只有 N 为奇数才构成 `\u` 转义。
 */
private val MALFORMED_UNICODE_ESCAPE = Regex("(\\\\+)u(?![0-9a-fA-F]{4})")

/**
 * 降级非法 `\u` 转义：只删掉引出转义的那一个反斜杠，前面的成对反斜杠原样保留。
 *
 * 反斜杠为偶数时 `u` 不是转义符，一个都不能动——动了反而把 `\\u12` 变成 `\u12`，让
 * JDK 抛「Malformed \uxxxx encoding」拒绝整份文件，把「写坏一个键」升级成「写坏一份
 * 配置」。
 */
private fun downgradeMalformedUnicodeEscapes(text: String): String =
    MALFORMED_UNICODE_ESCAPE.replace(text) { match ->
        val backslashes = match.groupValues[1]
        if (backslashes.length % 2 == 0) match.value else backslashes.dropLast(1) + "u"
    }

/**
 * 记录 [Properties] 的写入顺序与重复。
 *
 * `Properties.load` 逐条写入、后写覆盖先写，却**不保留**出现顺序与重复：同一个键写
 * 两遍在 `Properties` 里看不出来。本仓要把重复留痕（用户同时写两行通常是在排查
 * 为什么改了没效果），故自行记录每一次写入。
 */
private class RecordingProperties : Properties() {
    val recorded = mutableListOf<Pair<String, String>>()

    override fun put(key: Any?, value: Any?): Any? {
        // 值去首尾空白：Java Properties 保留尾随空白，而数值键不容忍尾随空白。
        recorded += key.toString() to value.toString().trim()
        return super.put(key, value)
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

        // 默认值只有一个来源：data class 的默认实例。此处再抄一份字面量时，改默认值
        // （例如边距从 3 改成 5）只有一处生效，另一处继续吐旧值，两份配置在
        // 「没写这个键」与「写了合法值」之间出现说不清的分叉。
        val defaults = TermuxProperties()
        val extraKeys =
            values["extra-keys"]?.let { raw ->
                when (val parsed = ExtraKeysParser.parse(raw)) {
                    is ExtraKeysParser.Result.Success -> {
                        errors += parsed.warnings
                        parsed.rows
                    }

                    is ExtraKeysParser.Result.Failure -> {
                        errors += "extra-keys: ${parsed.message}; keeping the default layout"
                        defaults.extraKeys
                    }
                }
            } ?: defaults.extraKeys

        val sessionShortcuts = buildMap {
            for (shortcut in SessionShortcut.entries) {
                val raw = values[shortcut.propertyKey] ?: continue
                val codePoint = parseCtrlShortcut(raw)
                if (codePoint == null) {
                    errors += "${shortcut.propertyKey}: '$raw' is not Ctrl+<character>; ignored"
                } else {
                    put(shortcut, codePoint)
                }
            }
        }
        // 上游还有 `shortcut.rename-session`，本应用没有重命名 UI（会话标题不可改）。
        // 它与「不认识的键」不同：用户从 termux 搬配置时多半带着这个键，静默丢掉会
        // 让人以为重命名快捷键在生效。故跳过它，但明确报告不支持。
        if (values.containsKey(UNSUPPORTED_RENAME_SHORTCUT)) {
            errors += "$UNSUPPORTED_RENAME_SHORTCUT: renaming sessions is not supported; ignored"
        }

        val properties =
            TermuxProperties(
                useFullscreenWorkaround = boolean("use-fullscreen-workaround", defaults.useFullscreenWorkaround),
                fullscreen = boolean("fullscreen", defaults.fullscreen),
                ctrlSpaceWorkaround = boolean("ctrl-space-workaround", defaults.ctrlSpaceWorkaround),
                hideSoftKeyboardOnStartup =
                boolean("hide-soft-keyboard-on-startup", defaults.hideSoftKeyboardOnStartup),
                extraKeysTextAllCaps = boolean("extra-keys-text-all-caps", defaults.extraKeysTextAllCaps),
                terminalMarginHorizontalDp =
                integer(
                    "terminal-margin-horizontal",
                    defaults.terminalMarginHorizontalDp,
                    TermuxProperties.MARGIN_RANGE,
                ),
                terminalMarginVerticalDp =
                integer(
                    "terminal-margin-vertical",
                    defaults.terminalMarginVerticalDp,
                    TermuxProperties.MARGIN_RANGE,
                ),
                terminalCursorStyle =
                when ((values["terminal-cursor-style"] ?: "block").lowercase()) {
                    "block" -> TerminalCursorStyle.BLOCK

                    // 只接受上游的三个取值：别名（如 `beam`）不是配置契约，
                    // 多认一个值就多一份与上游不一致的可能。
                    "bar" -> TerminalCursorStyle.BAR

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
                    defaults.terminalTranscriptRows,
                    TermuxProperties.TRANSCRIPT_ROWS_RANGE,
                ),
                terminalToolbarHeightScale =
                float(
                    "terminal-toolbar-height",
                    defaults.terminalToolbarHeightScale,
                    TermuxProperties.TOOLBAR_HEIGHT_SCALE_RANGE,
                ),
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
                sessionShortcuts = sessionShortcuts,
            )
        return TermuxPropertiesLoad(properties, errors)
    }

    /**
     * `Ctrl+<字符>` → 码点；格式不对返回 null。
     *
     * 上游 `TermuxPropertyConstants` 的写法是 `Ctrl+n`（大小写不敏感，允许空格），
     * 只接受两个分量且第二段恰好一个码点；其他键名（`Alt+n`）上游也解析，但本应用
     * 只处理 Ctrl，故明确拒绝。
     */
    private fun parseCtrlShortcut(raw: String): Int? {
        val parts = raw.split('+').map { it.trim() }
        if (parts.size != 2 || !parts[0].equals("ctrl", ignoreCase = true)) return null
        val character = parts[1]
        if (character.isEmpty() || character.codePointCount(0, character.length) != 1) return null
        return Character.toLowerCase(character.codePointAt(0))
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
 * `popup` 是**完整的按键**（上游 `ExtraKeysInfo.normalizeKeyConfig` 把字符串包成
 * `{key: 'X'}`，对象原样成为次级按键），故对象形式必须解析出键名与宏。
 *
 * 写坏的项 MUST 只丢自己那一项：整份布局因为一个不认识的字段回到默认，等于用户
 * 精心配的两行键位被一个对象整体作废。
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
            val start = index
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
            error("unterminated string starting at offset $start")
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

        /** 对象字段名：上游 JSON 同时接受裸名与带引号的名字。 */
        fun readName(): String {
            skipWhitespace()
            return when (text.getOrNull(index)) {
                '\'', '"' -> readString()
                else -> readBare()
            }
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

        /**
         * 把光标移到从 [start] 起的那段复合值的闭合符**之后**。
         *
         * 用于坏掉的对象只丢自己那一项时的恢复：光标必须跳过这个坏对象，否则后面的
         * 键位会被错位解析。结构本身不完整（缺闭合符）时抛出——那种配置无从可靠恢复，
         * 整份布局退回默认。
         */
        fun skipPastComposite(start: Int) {
            index = start
            skipComposite()
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

    /**
     * 解析一个按键项。
     *
     * 对象里的语法错误（缺冒号、多余符号）只丢这一项：把光标同步到该对象的闭合符
     * 之后，其余键位照常解析。整份布局因为一个手误的冒号全部消失，正是「只丢一项」
     * 约定要防的情形。
     */
    private fun parseKey(cursor: Cursor): ExtraKey? {
        if (cursor.peek() == '{') {
            val objectStart = cursor.index
            cursor.consume('{')
            return try {
                parseKeyObject(cursor, "key object", allowPopup = true)
            } catch (error: IllegalArgumentException) {
                dropMalformedObject(cursor, objectStart, error)
            } catch (error: IllegalStateException) {
                dropMalformedObject(cursor, objectStart, error)
            }
        }
        return ExtraKey(cursor.readString())
    }

    /** 坏掉的对象只丢自己：同步光标并留下说明。结构不完整时抛出，整份布局退回默认。 */
    private fun dropMalformedObject(cursor: Cursor, objectStart: Int, error: RuntimeException): ExtraKey? {
        cursor.skipPastComposite(objectStart)
        cursor.warnings += "extra-keys: malformed key object dropped (${error.message})"
        return null
    }

    /**
     * 解析一个按键对象的内容（`{` 已由调用方消费，`}` 在此消费）。
     *
     * @param context 警告里的称呼（`key object` / `popup object`）。
     * @param allowPopup 是否接受嵌套的 `popup`。上游的 popup 对象不再带 popup
     *   （`ExtraKeysInfo` 只取它的 `key`/`macro`/`display`），故为 false。
     *
     * 返回 null 表示这一项不可用：调用方按上下文决定是丢掉整项（顶层按键）还是只
     * 丢掉这个 popup（次级按键），两种情况下其余键位都照常保留。
     */
    private fun parseKeyObject(cursor: Cursor, context: String, allowPopup: Boolean): ExtraKey? {
        var key: String? = null
        var popup: ExtraKey? = null
        var macro: String? = null
        var display: String? = null
        var fieldCount = 0
        while (true) {
            // 尾随逗号（`{key: 'X',}`）与空对象（`{}`）都收：上游的 JSON 解析器同样
            // 收这两种写法，为一个逗号丢掉整个键位没有道理。尾随逗号留一条说明。
            if (cursor.peek() == '}') {
                if (fieldCount > 0) {
                    cursor.warnings += "extra-keys: $context has a trailing comma"
                }
                break
            }
            val name = cursor.readName()
            cursor.consume(':')
            fieldCount++
            when (name) {
                "key" -> key = cursor.readValueOrNull()

                "popup" ->
                    if (allowPopup) {
                        popup = parsePopup(cursor)
                    } else {
                        cursor.readValueOrNull()
                        cursor.warnings += "extra-keys: nested popup inside a popup object is ignored"
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
        if (key != null && macro != null) {
            // 上游直接抛错（`ExtraKeyButton` 构造函数的检查），本应用只丢这一项。
            cursor.warnings += "extra-keys: $context sets both 'key' and 'macro'; dropped"
            return null
        }
        if (key.isNullOrEmpty() && macro == null) {
            // `display` 只是键帽文字，没有可执行内容：丢掉这一项，其余按键照常保留。
            cursor.warnings += "extra-keys: $context without 'key' or 'macro' dropped (display is only a label)"
            return null
        }
        return ExtraKey(
            // 只有宏的对象没有键名，用 display 顶上（宏本身才是它的可执行内容）。
            name = key ?: display.orEmpty(),
            popup = popup,
            macro = macro,
            display = display,
        )
    }

    /**
     * `popup` 的值：键名字符串，或按键对象（可含 `macro`）。
     *
     * 上游的 popup 是一个完整的按键：字符串被包成 `{key: 'X'}`，对象原样成为次级
     * 按键。故对象形式必须解析，不能当作「不认识的字段」丢掉。
     */
    private fun parsePopup(cursor: Cursor): ExtraKey? = when (cursor.peek()) {
        '{' -> {
            val objectStart = cursor.index
            cursor.consume('{')
            try {
                parseKeyObject(cursor, "popup object", allowPopup = false)
            } catch (error: IllegalArgumentException) {
                dropMalformedPopup(cursor, objectStart, error)
            } catch (error: IllegalStateException) {
                dropMalformedPopup(cursor, objectStart, error)
            }
        }

        '[' -> {
            cursor.skipComposite()
            cursor.warnings += "extra-keys: popup must be a key or a key object; dropped"
            null
        }

        else -> ExtraKey(name = cursor.readString())
    }

    /**
     * 坏掉的 popup 对象只丢这个 popup：主键照常保留。
     *
     * popup 是次级键（长按才触发），主键才是日常使用的那一个。popup 里少一个逗号
     * 就把主键一起丢掉，等于用户为一个长按动作赔上整个键位——与「写坏的项只丢自己
     * 那一项」的约定直接冲突（该约定的原文见本对象的类文档）。
     *
     * 对象本身不完整（缺闭合符）时 `skipPastComposite` 仍会抛出：那种配置无从可靠
     * 恢复，整份布局退回默认。
     */
    private fun dropMalformedPopup(cursor: Cursor, objectStart: Int, error: RuntimeException): ExtraKey? {
        cursor.skipPastComposite(objectStart)
        cursor.warnings += "extra-keys: malformed popup object dropped (${error.message})"
        return null
    }
}
