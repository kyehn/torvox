package terminal.emulator.settings

/**
 * `~/.termux/termux.properties` 的解析结果。
 *
 * 字段语义与默认值取自 termux-app `TermuxPropertyConstants`，仅实现本应用实际支持的键；
 * 上游存在但本应用无法正确实现的键不解析（见 `docs/specification/config.md`）。
 *
 * 解析失败一律「保留默认值 + 记录错误」，不让一个写错的键把整份配置作废，更不因
 * 用户文件而崩溃——这是外部配置，不是应用自己的设置数据。
 */
data class TermuxProperties(
    val fullscreen: Boolean = false,
    val ctrlSpaceWorkaround: Boolean = false,
    /**
     * 启动时不弹软键盘。
     *
     * 默认 `true`，与上游不同（上游默认 `false` = 启动即弹）：本应用启动时从不主动
     * 请求输入法，默认值必须反映既有外观。写 `false` 会在首帧后等窗口取得焦点再请求
     * 一次输入法，避免 `show` 在无焦点窗口上被平台丢弃。
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
         * 固定两行布局：`ESC DRAWER SCROLL HOME ↑ END PGUP` / `TAB CTRL ALT ← ↓ → PGDN`。
         *
         * **不是** termux 上游的 `DEFAULT_IVALUE_EXTRA_KEYS`（上游是
         * `ESC / - HOME ↑ END PGUP`，没有 DRAWER 与 SCROLL）：在 termux 里抽屉由
         * 左缘滑动打开、且它没有滚动锁定，而本应用把这两者做成了键栏上的固定入口。
         * 换成上游默认会让「打开会话列表」和「锁定滚动位置」两个功能失去唯一入口。
         *
         * 列数守 DESIGN.md 修饰键栏节的「固定 2 行 7 列」。`extra-keys` 不解析，
         * 用户无法改动键位；`extra-keys-style` 与 `extra-keys-text-all-caps` 只改字形。
         */
        val DEFAULT_EXTRA_KEYS: List<List<String>> =
            listOf(
                listOf("ESC", "DRAWER", "SCROLL", "HOME", "UP", "END", "PGUP"),
                listOf("TAB", "CTRL", "ALT", "LEFT", "DOWN", "RIGHT", "PGDN"),
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

/** 一份配置的来源文件与解析错误，供日志与诊断使用。 */
data class TermuxPropertiesLoad(val properties: TermuxProperties, val sourcePath: String?, val errors: List<String>)

/**
 * Java Properties 风格的读取器：`key = value` / `key: value`，`#` 与 `!` 起注释，
 * 行尾反斜杠续行，支持 `\n` `\t` `\\` `\ ` `\uXXXX` 转义。
 *
 * 不用 `java.util.Properties`：分隔符只在首个未转义的 `=`/`:` 处切一次，
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

        fun choice(key: String, default: String): String {
            val raw = values[key] ?: return default
            return raw
        }

        val properties =
            TermuxProperties(
                fullscreen = boolean("fullscreen", false),
                ctrlSpaceWorkaround = boolean("ctrl-space-workaround", false),
                hideSoftKeyboardOnStartup = boolean("hide-soft-keyboard-on-startup", true),
                extraKeysTextAllCaps = boolean("extra-keys-text-all-caps", true),
                terminalMarginHorizontalDp = integer("terminal-margin-horizontal", 3, 0..100),
                terminalMarginVerticalDp = integer("terminal-margin-vertical", 0, 0..100),
                terminalCursorStyle =
                when (choice("terminal-cursor-style", "block").lowercase()) {
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
                when (choice("extra-keys-style", "default").lowercase()) {
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
                defaultWorkingDirectory = values["default-working-directory"]?.trim().orEmpty(),
                backKey =
                when (choice("back-key", "back").lowercase()) {
                    "back" -> BackKeyBehaviour.BACK

                    "escape" -> BackKeyBehaviour.ESCAPE

                    else -> {
                        errors += "back-key: '${values["back-key"]}' is unknown; keeping back"
                        BackKeyBehaviour.BACK
                    }
                },
                volumeKeys =
                when (choice("volume-keys", "virtual").lowercase()) {
                    "virtual" -> VolumeKeysBehaviour.VIRTUAL

                    "volume" -> VolumeKeysBehaviour.VOLUME

                    else -> {
                        errors += "volume-keys: '${values["volume-keys"]}' is unknown; keeping virtual"
                        VolumeKeysBehaviour.VIRTUAL
                    }
                },
                softKeyboardToggleBehaviour =
                when (choice("soft-keyboard-toggle-behaviour", "show/hide").lowercase()) {
                    "show/hide" -> SoftKeyboardToggleBehaviour.SHOW_HIDE

                    "enable/disable" -> SoftKeyboardToggleBehaviour.ENABLE_DISABLE

                    else -> {
                        errors +=
                            "soft-keyboard-toggle-behaviour: '${values["soft-keyboard-toggle-behaviour"]}' is unknown; using show/hide"
                        SoftKeyboardToggleBehaviour.SHOW_HIDE
                    }
                },
            )
        return TermuxPropertiesLoad(properties, null, errors)
    }
}
