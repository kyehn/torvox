package terminal.emulator.benchmark

/*
 * 突发输出场景的负载：一次性灌入终端的大块字节流。
 *
 * 负载是设备侧 shell 自己产的字节流，故字节流不依赖宿主的注入速度，各被测应用拿到
 * 的是同一串字节。突发与持续输出的区别在节奏：这里整段一次灌完，中间没有节流，
 * 终端必须把它整段吞下并持续画出来。
 *
 * 流的末尾带两个哨兵，顺序即证据链：
 *
 * - 会话标题哨兵（OSC 2）：它写在整段字节流之后，终端按序解析，标题出现在应用
 *   状态里即证明终端把整段流解析到了末尾——中途丢字节时标题不会出现。
 * - 完成标记（`log -t` 写进 logcat）：生产者把全部字节写进了 PTY。
 *
 * 提示符默认每次都改写会话标题（mkshrc 的 PS1 带 OSC 0），哨兵会被立刻覆盖；测量
 * 前把提示符换成不含转义的普通提示符，哨兵才留得住。
 */

/** 完成标记的 logcat 标签。 */
internal const val BURST_OUTPUT_MARKER_TAG = "s6out"

/** 完成标记的正文。 */
internal const val BURST_OUTPUT_MARKER = "__s6done"

/** 会话标题哨兵：整段字节流之后由应用状态显形，证明终端消费到了流末尾。 */
internal const val BURST_OUTPUT_CANARY = "__s6tail"

/**
 * 突发输出的负载描述。
 *
 * 命令只用 `seq`、`printf`、`log` 三个外部命令：各被测应用的 shell 都能跑，负载
 * 形状不依赖 shell 的算术或条件语法。
 */
internal data class BurstOutputLoad(
    /** 突发输出的行数（批内产出 `seq 1 <行数>`）。 */
    val lineCount: Int,
    /** 流末尾写入的会话标题哨兵。 */
    val canary: String,
) {
    init {
        require(lineCount > 0) { "突发行数必须为正：$lineCount" }
        require(canary.isNotEmpty()) { "标题哨兵不能为空" }
    }

    /** 负载命令：整段字节流写完后依次写标题哨兵与完成标记。 */
    fun command(): String = "seq 1 $lineCount; printf '\\033]2;$canary\\007'; " +
        "log -t $BURST_OUTPUT_MARKER_TAG $BURST_OUTPUT_MARKER"

    /**
     * 喂给 `input text` 的参数：空格写成 `%s`（`input text` 的空格转义），其余原样。
     *
     * 设备侧命令不经 shell，空格会把参数切成多段，故空格必须先转义。
     */
    fun inputTextArgument(): String = command().replace(" ", "%s")

    /**
     * 突发送达终端的字节数：行内容加每个换行被 PTY 展开成的 CRLF，再加标题哨兵。
     *
     * PTY 的输出处理把每个 `\n` 展开成 `\r\n`（行规程的默认行为），终端实际收到的
     * 字节数因此是行内容的字节数加两倍行数。标题哨兵是 `ESC ] 2 ; <哨兵> BEL`，
     * 共哨兵字节数加五字节定长部分。
     */
    fun bytes(): Long {
        var payloadBytes = 0L
        var firstLine = 1L
        var digits = 1
        while (firstLine <= lineCount) {
            val lastLine = minOf(firstLine * 10 - 1, lineCount.toLong())
            payloadBytes += (lastLine - firstLine + 1) * digits
            firstLine *= 10
            digits++
        }
        return payloadBytes + 2L * lineCount + canary.length + 5L
    }
}
