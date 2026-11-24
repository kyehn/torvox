package terminal.emulator.benchmark

/**
 * S8 宽字符行的负载：把宽字符写成 UTF-8 字节的八进制转义，设备侧 shell 用 `printf`
 * 还原成宽字符输出。宽字符不能直接敲进终端（硬件键盘事件通道按 KeyCharacterMap
 * 映射字符，非 ASCII 映射不到），故必须走八进制转义。
 *
 * 五个模板：CJK、组合附加符、emoji、ZWJ 家庭、全角拉丁；`octalEscaped()` 生成定长
 * 三位八进制转义；`command()` 设备侧 `for` 循环重复输出；`inputTextArgument()` 把空格
 * 转成 `%s`；`bytes()` 按 PTY 展开 CRLF 计算字节数。
 */
internal class WideCharacterLoad(
    val lineCount: Int,
    val canary: String,
    private val completion: String = "log -t $WIDE_CHARACTER_COUNT_TAG $WIDE_CHARACTER_MARKER",
) {
    init {
        require(lineCount > 0) { "轮数必须为正：$lineCount" }
        require(canary.isNotEmpty()) { "标题哨兵不能为空" }
    }

    /** 宽字符模板行。 */
    private val lines: List<String> = WIDE_CHARACTER_LINES

    /** 一行模板的 UTF-8 字节数。 */
    private fun templateBytes(): Int = lines.sumOf { it.toByteArray(Charsets.UTF_8).size }

    /** 把一个字符写成定长三位八进制转义。 */
    private fun octalEscaped(text: String): String = text.toByteArray(Charsets.UTF_8).joinToString("") { byte ->
        "\\%03o".format(byte.toInt() and 0xFF)
    }

    /** 设备侧 shell 命令：循环输出模板行，再写标题哨兵，最后打完成标记。 */
    fun command(): String {
        val format = lines.joinToString("") { line -> octalEscaped(line) + "\\n" }
        return "for i in \$(seq 1 $lineCount); do printf '$format'; done; " +
            "printf '\\033]2;$canary\\007'; $completion"
    }

    /** 适配 `input text` 的参数：空格转 `%s`。 */
    fun inputTextArgument(): String = command().replace(" ", "%s")

    /** 负载字节数：PTY 展开 CRLF，故换行按两字节算。 */
    fun bytes(): Long {
        val templateBytes = lines.sumOf { it.toByteArray(Charsets.UTF_8).size.toLong() }
        val lineBytes = templateBytes + 2L * lines.size
        return lineBytes * lineCount + canary.length + 5L
    }
}

/** 五个宽字符模板。 */
internal val WIDE_CHARACTER_LINES: List<String> = listOf(
    "你好世界",
    "e\u0301",
    "😀",
    "👨\u200D👩\u200D👧",
    "ＡＢＣ",
)
