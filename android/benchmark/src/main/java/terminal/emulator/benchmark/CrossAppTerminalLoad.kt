package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.UiDevice

/*
 * 跨应用输出负载的注入与完成判定。
 *
 * 被测应用换成第三方终端后，应用内的完成标记（logcat 标签、会话标题、原生呈现汇总）
 * 都不再存在，故负载与信号都必须与被测应用无关：命令经 `input text` 敲进终端，完成
 * 标记由应用的 shell 写成 /sdcard 上的文件（系统侧可读）。两个被测应用拿到完全相同
 * 的命令与落盘路径，跨应用对比才成立。
 */

/** 就绪标记的正文。 */
internal const val CROSS_APP_READY_MARKER = "READY"

/** 完成标记的正文。 */
internal const val CROSS_APP_DONE_MARKER = "DONE"

/** 设备侧文件的内容；文件不存在或为空时返回 null。 */
internal fun readDeviceFile(device: UiDevice, path: String): String? =
    device.executeShellCommand("cat $path").trim().takeIf { it.isNotEmpty() }

/** 删除设备侧文件：每轮开始前清掉上一轮的标记，避免把旧完成信号当成本轮。 */
internal fun deleteDeviceFile(device: UiDevice, path: String) {
    device.executeShellCommand("rm -f $path")
}

/**
 * 把一行命令敲进当前获得输入焦点的终端。
 *
 * `input text` 的空格转义是 `%s`；回车单独发（命令文本里带换行会被切成多次输入）。
 */
internal fun typeTerminalCommand(device: UiDevice, command: String) {
    device.executeShellCommand("input text ${command.replace(" ", "%s")}")
    device.executeShellCommand("input keyevent 66")
}

/** 终端网格尺寸（行 × 列）。 */
internal data class TerminalGrid(val rows: Int, val columns: Int) {
    /** 一行式说明，进实测行与日志。 */
    fun describe(): String = "${rows}x$columns"
}

/** 解析 `stty size` 的输出（`<行> <列>`）；格式不对返回 null。 */
internal fun parseTerminalGrid(output: String?): TerminalGrid? {
    val columns = output?.trim()?.split(WHITESPACE) ?: return null
    if (columns.size != 2) return null
    val rows = columns[0].toIntOrNull() ?: return null
    val width = columns[1].toIntOrNull() ?: return null
    if (rows <= 0 || width <= 0) return null
    return TerminalGrid(rows, width)
}

/**
 * 一轮跨应用负载的落盘路径。
 *
 * 同一轮的两个被测应用共用同一组路径：路径是命令的一部分，路径不同则命令不同。
 * 每轮开始前清空，故共路径不会把上一轮的标记算进本轮。
 */
internal data class CrossAppLoadPaths(val runId: String) {
    val readyPath: String get() = "/sdcard/Download/bench_${runId}_ready"

    val gridPath: String get() = "/sdcard/Download/bench_${runId}_grid"

    val donePath: String get() = "/sdcard/Download/bench_${runId}_done"
}

/** 就绪探针：写网格尺寸与就绪标记；shell 尚未就绪时重发。 */
internal fun readyProbeCommand(paths: CrossAppLoadPaths): String =
    "stty size > ${paths.gridPath}; printf $CROSS_APP_READY_MARKER > ${paths.readyPath}"

/**
 * 等终端 shell 就绪：重发探针直到就绪标记出现。
 *
 * 就绪信号取「shell 真的执行了命令」——PTY 建好不等于 shell 在读输入，初始化 tty 时会
 * 冲掉先到的输入，故重发；重发是幂等的（覆盖同一文件）。超时即失败，不把「没就绪」
 * 当成通过。
 */
internal fun awaitTerminalShellReady(
    device: UiDevice,
    paths: CrossAppLoadPaths,
    timeoutMillis: Long,
    budget: ScenarioBudget? = null,
) {
    deleteDeviceFile(device, paths.readyPath)
    val deadline = SystemClock.elapsedRealtime() + effectiveTimeoutMillis(timeoutMillis, budget)
    var nextProbeMillis = 0L
    while (SystemClock.elapsedRealtime() < deadline) {
        if (readDeviceFile(device, paths.readyPath) == CROSS_APP_READY_MARKER) return
        val now = SystemClock.elapsedRealtime()
        if (now >= nextProbeMillis) {
            typeTerminalCommand(device, readyProbeCommand(paths))
            nextProbeMillis = now + SHELL_READY_PROBE_INTERVAL_MILLIS
        }
        SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
    }
    error("终端 shell 未就绪：${timeoutMillis}ms 内没有写出就绪标记")
}

/** 读取终端网格；读不到即失败——网格不等价的对比无意义。 */
internal fun readTerminalGrid(device: UiDevice, paths: CrossAppLoadPaths): TerminalGrid {
    val output = readDeviceFile(device, paths.gridPath)
    return checkNotNull(parseTerminalGrid(output)) { "读不到终端网格尺寸（stty size 输出：$output）" }
}

/** 超时取场景预算与给定上限的较小值，预算耗尽即不再等待。 */
internal fun effectiveTimeoutMillis(timeoutMillis: Long, budget: ScenarioBudget?): Long =
    budget?.let { minOf(timeoutMillis, it.remainingMillis()) } ?: timeoutMillis

/**
 * S5 跨应用持续输出负载：逐批产出 `seq 1 <每批行数>`，批间停 [pauseSeconds]，
 * 末批后写完成标记。
 *
 * 命令只有 `for` 循环与 `seq`、`sleep`、`printf`，不依赖 shell 的算术或条件语法，
 * 各家 shell 都能跑；命令文本与落盘路径对两个被测应用完全相同。
 */
internal fun sustainedOutputCommand(
    paths: CrossAppLoadPaths,
    batchCount: Int,
    linesPerBatch: Int,
    pauseSeconds: String,
): String = buildString {
    append("for b in \$(seq 1 ")
    append(batchCount)
    append("); do seq 1 ")
    append(linesPerBatch)
    append("; sleep ")
    append(pauseSeconds)
    append("; done; printf ")
    append(CROSS_APP_DONE_MARKER)
    append(" > ")
    append(paths.donePath)
}

/** 逐图层呈现时刻的采样结果。 */
internal data class LayerPresentSample(
    /** 负载是否在超时内写出了完成标记。 */
    val completed: Boolean,
    /** 采样到的呈现时刻（纳秒，绝对单调时刻，升序去重）。 */
    val presentTimesNanos: List<Long>,
)

/**
 * 采样一段窗口内的逐图层呈现时刻，直到完成标记出现或超时。
 *
 * 系统侧只保留最近一段帧历史，单次读只能看到窗口尾部；轮询并把呈现时刻并入集合
 * 才能覆盖整个输出过程。完成标记是负载结束的唯一判据，超时返回未完成由调用方判红。
 */
internal fun sampleLayerPresentsUntilMarker(
    device: UiDevice,
    layerName: String,
    paths: CrossAppLoadPaths,
    timeoutMillis: Long,
    pollMillis: Long,
    budget: ScenarioBudget? = null,
): LayerPresentSample {
    val presents = sortedSetOf<Long>()
    val deadline = SystemClock.elapsedRealtime() + effectiveTimeoutMillis(timeoutMillis, budget)
    while (true) {
        readLayerFrameStats(device, layerName)?.let { stats -> presents += stats.presentTimesNanos }
        val completed = readDeviceFile(device, paths.donePath) == CROSS_APP_DONE_MARKER
        if (completed || SystemClock.elapsedRealtime() >= deadline) {
            return LayerPresentSample(completed, presents.toList())
        }
        budget?.checkDeadline()
        SystemClock.sleep(pollMillis)
    }
}

/** 在一段固定窗口内采样逐图层呈现时刻（跨整段窗口，避免只看到保留窗口的尾部）。 */
internal fun sampleLayerPresents(
    device: UiDevice,
    layerName: String,
    windowMillis: Long,
    pollMillis: Long,
    budget: ScenarioBudget? = null,
): List<Long> {
    val presents = sortedSetOf<Long>()
    val deadline = SystemClock.elapsedRealtime() + effectiveTimeoutMillis(windowMillis, budget)
    while (true) {
        readLayerFrameStats(device, layerName)?.let { stats -> presents += stats.presentTimesNanos }
        if (SystemClock.elapsedRealtime() >= deadline) return presents.toList()
        budget?.checkDeadline()
        SystemClock.sleep(pollMillis)
    }
}

/** 一段呈现时刻窗口内的帧面统计。 */
internal data class LayerWindowStats(
    /** 窗口内的呈现帧数。 */
    val frameCount: Int,
    /** 呈现间隔的 p95（毫秒）；样本不足时为 0。 */
    val p95IntervalMillis: Long,
    /** 呈现占用的刷新槽位比例；样本不足时为 0。 */
    val presentationSlotRatio: Double,
)

/**
 * 按绝对时刻窗口统计呈现帧面。
 *
 * 逐图层通道给的是绝对单调时钟纳秒，与 `elapsedRealtime` 同域，故窗口直接按纳秒比较，
 * 不需要像显示级帧时间线那样换算基准。
 */
internal fun layerWindowStats(
    presentTimesNanos: List<Long>,
    windowStartNanos: Long,
    windowEndNanos: Long,
    refreshPeriodNanos: Long,
): LayerWindowStats {
    val window = presentTimesNanos.filter { it in windowStartNanos..windowEndNanos }
    if (window.size < 2) return LayerWindowStats(window.size, 0L, 0.0)
    val intervalsNanos = window.zipWithNext { previous, current -> current - previous }
    val spanNanos = window.last() - window.first()
    val ratePerSecond = if (spanNanos > 0) intervalsNanos.size * 1_000_000_000.0 / spanNanos else 0.0
    return LayerWindowStats(
        frameCount = window.size,
        p95IntervalMillis = percentile(intervalsNanos, 0.95) / 1_000_000,
        presentationSlotRatio = ratePerSecond * refreshPeriodNanos / 1_000_000_000.0,
    )
}
