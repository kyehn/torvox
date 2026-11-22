package terminal.emulator.benchmark

/*
 * 持续输出场景的负载：固定批数、每批固定行数、批间固定间隔。
 *
 * 负载是设备侧 shell 自己产的字节流，故字节流不依赖宿主的注入速度，各被测应用
 * 拿到的是同一串字节。批间停固定时间，输出在整个窗口里持续到达——一次性灌进来的
 * 大块字节流是突发输出场景的负载，两者不是一回事。
 *
 * 每批结束打一个完成标记（写到 logcat 的固定标签），批次是否真的跑完由此判定：
 * 终端吞掉输出、shell 半途死掉时，呈现帧数不为零也证明不了字节流完整。标记同时
 * 给出每批的到达时刻，吞吐与停摆间隔据此算出。
 */

/** 完成标记的 logcat 标签。 */
internal const val SUSTAINED_OUTPUT_MARKER_TAG = "s5out"

/** 完成标记的正文前缀，后面跟批号。 */
internal const val SUSTAINED_OUTPUT_MARKER_PREFIX = "__s5b"

/**
 * 完成标记的匹配：正文形如 `__s5b7`。
 *
 * 过滤词 MUST 不含空白：设备侧命令不经 shell，按空白切词。
 */
internal val SUSTAINED_OUTPUT_MARKER_PATTERN = Regex("$SUSTAINED_OUTPUT_MARKER_PREFIX[0-9]+")

/**
 * 持续输出的负载描述。
 *
 * 批内是一次 `seq` 的完整输出，批末打完成标记：命令里只有 `for` 循环与 `seq`、
 * `sleep`、`log` 三个外部命令，不依赖 shell 的算术或条件语法，各家 shell 都能跑。
 */
internal data class SustainedOutputLoad(
    /** 批次数。 */
    val batchCount: Int,
    /** 每批的行数（批内产出 `seq 1 <每批行数>`）。 */
    val linesPerBatch: Int,
    /** 批间隔（秒）。 */
    val pauseSeconds: String,
) {
    init {
        require(batchCount > 0) { "批次数必须为正：$batchCount" }
        require(linesPerBatch > 0) { "每批行数必须为正：$linesPerBatch" }
    }

    /** 负载命令：逐批产出固定行数，批末打一个完成标记。 */
    fun command(): String = buildString {
        append("for i in ")
        append((1..batchCount).joinToString(" "))
        append("; do seq 1 ")
        append(linesPerBatch)
        append("; sleep ")
        append(pauseSeconds)
        append("; log -t ")
        append(SUSTAINED_OUTPUT_MARKER_TAG)
        append(' ')
        append(SUSTAINED_OUTPUT_MARKER_PREFIX)
        append("${'$'}i; done")
    }

    /**
     * 喂给 `input text` 的参数：空格写成 `%s`（`input text` 的空格转义），其余原样。
     *
     * 设备侧命令不经 shell，空格会把参数切成多段，故空格必须先转义。
     */
    fun inputTextArgument(): String = command().replace(" ", "%s")

    /**
     * 每批送达终端的字节数：行内容加每个换行被 PTY 展开成的 CRLF。
     *
     * PTY 的输出处理把每个 `\n` 展开成 `\r\n`（行规程的默认行为），终端实际收到
     * 的字节数因此是行内容的字节数加两倍行数。
     */
    fun bytesPerBatch(): Long {
        var payloadBytes = 0L
        for (line in 1..linesPerBatch) payloadBytes += line.toString().length
        return payloadBytes + 2L * linesPerBatch
    }
}
