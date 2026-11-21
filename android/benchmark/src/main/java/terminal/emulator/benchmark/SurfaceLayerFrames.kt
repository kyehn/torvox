package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice
import kotlin.math.max
import kotlin.math.roundToInt

/*
 * 系统侧逐图层帧时序（`dumpsys SurfaceFlinger --latency <图层名>`）。
 *
 * 终端内容由原生渲染线程直绘 SurfaceView 的 Surface，不经主线程 `ViewRootImpl`，
 * `dumpsys gfxinfo` 记不到这些帧（见 [PresentedFrames]）。系统侧逐图层通道是与
 * 对照应用同口径的终端内容帧面：它按图层给出每帧的呈现时刻，故滚动流畅度与
 * 跨应用比较都取自这里。
 *
 * 设备侧命令不经 shell（`UiAutomation` 走 `Runtime.exec(String)`，按空白切词），
 * 故图层名不能加引号：引号会原样进入参数，图层名便匹配不上。含空白的图层名
 * （如 `Background for SurfaceView[...]`）在这条通道上本就不可寻址。
 */

/** 承载终端内容的图层名前缀：本应用 SurfaceView 的图层。 */
private const val SURFACE_VIEW_LAYER_PREFIX = "SurfaceView[$TARGET_PACKAGE/$TARGET_ACTIVITY]"

/** `--latency` 每帧的列数：期望呈现、实际呈现、就绪时刻。 */
private const val LAYER_FRAME_COLUMN_COUNT = 3

/** 空白切分：图层清单与帧记录都按空白分列。 */
private val WHITESPACE = Regex("\\s+")

/**
 * 系统侧逐图层帧时序的统计。
 *
 * 帧数是保留窗口内的观测值，不是场景总帧数：系统侧只保留最近一段历史（实测每图层
 * 127 行），持续输出的场景会把它截断，故帧数只作「系统侧确实看到了内容帧」的证据，
 * 总帧数仍取应用自己的计数口径。
 */
internal data class LayerFrameStats(
    /** 显示刷新周期（纳秒），`--latency` 首行给出。 */
    val refreshPeriodNanos: Long,
    /** 有内容帧的记录数（保留窗口内的观测值）。 */
    val contentFrameCount: Int,
    /** 呈现间隔跨过刷新槽位折算出的丢帧数。 */
    val missedFrameCount: Int,
    /** 呈现间隔的中位数（毫秒）。 */
    val medianIntervalMillis: Long,
    /** 呈现间隔的 p90（毫秒）。 */
    val p90IntervalMillis: Long,
    /** 呈现间隔的最大值（毫秒）。 */
    val maxIntervalMillis: Long,
    /** 呈现频率（帧/秒）：相邻呈现时刻之差折算。 */
    val presentationRatePerSecond: Double,
) {
    /**
     * 呈现占用的刷新槽位比例：呈现频率除以设备刷新率。
     *
     * 这是与设备刷新率无关的无量纲量。丢帧数是保留窗口内的绝对量，会随窗口内帧数
     * 一起变大；只有「刷了多少个槽位」能跨设备刷新率比较。比例可能略大于 1：系统侧
     * 记的是实际呈现时刻，未严格落在刷新槽位边界上。
     */
    val presentationSlotRatio: Double
        get() = presentationRatePerSecond * refreshPeriodNanos / 1_000_000_000.0
}

/**
 * 解析 `--latency` 输出。
 *
 * 首行是刷新周期（纳秒），其后每行三列：期望呈现、实际呈现、就绪时刻（纳秒）。
 * 全零行表示该槽位没有内容帧。没有内容帧时返回 null——图层匹配却没有帧记录即
 * 采集失败，调用方须判红，而不是把它当成「被测路径没有出帧」。
 */
internal fun parseLayerFrameStats(output: String): LayerFrameStats? {
    val lines = output.lineSequence().map { it.trim() }.filter { it.isNotEmpty() }.toList()
    val refreshPeriodNanos = lines.firstOrNull()?.toLongOrNull() ?: return null
    if (refreshPeriodNanos <= 0) return null
    val presentTimesNanos = lines.drop(1).mapNotNull { line ->
        val columns = line.split(WHITESPACE)
        if (columns.size != LAYER_FRAME_COLUMN_COUNT) return@mapNotNull null
        val actualPresentNanos = columns[1].toLongOrNull() ?: return@mapNotNull null
        actualPresentNanos.takeIf { it > 0 }
    }
    if (presentTimesNanos.isEmpty()) return null
    val intervalsNanos = presentTimesNanos.zipWithNext { previous, current -> current - previous }
    val sortedIntervalsNanos = intervalsNanos.sorted()
    val spanNanos = presentTimesNanos.last() - presentTimesNanos.first()
    return LayerFrameStats(
        refreshPeriodNanos = refreshPeriodNanos,
        contentFrameCount = presentTimesNanos.size,
        missedFrameCount = intervalsNanos.sumOf { intervalNanos ->
            max(0, (intervalNanos.toDouble() / refreshPeriodNanos).roundToInt() - 1)
        },
        medianIntervalMillis = percentileMillis(sortedIntervalsNanos, 0.5),
        p90IntervalMillis = percentileMillis(sortedIntervalsNanos, 0.9),
        maxIntervalMillis = sortedIntervalsNanos.lastOrNull()?.div(1_000_000) ?: 0L,
        presentationRatePerSecond = if (spanNanos > 0) {
            (intervalsNanos.size * 1_000_000_000.0) / spanNanos
        } else {
            0.0
        },
    )
}

/** 百分位取值：下标取 `floor(比例 × 样本数)` 并夹到端点，样本为空时取 0。 */
private fun percentileMillis(sortedValuesNanos: List<Long>, ratio: Double): Long {
    if (sortedValuesNanos.isEmpty()) return 0L
    val index = (ratio * sortedValuesNanos.size).toInt().coerceIn(0, sortedValuesNanos.size - 1)
    return sortedValuesNanos[index] / 1_000_000
}

/**
 * 找出承载终端内容的图层名。
 *
 * 候选是本应用 SurfaceView 的图层；逐个查帧记录，取第一个带内容帧的。BLAST 模式下
 * 内容在带 `(BLAST)` 的子层，父层不带内容帧（实测父层帧记录全为零）。
 */
internal fun findTerminalContentLayer(device: UiDevice): String {
    val candidates = listLayerNames(device).filter { it.startsWith(SURFACE_VIEW_LAYER_PREFIX) }
    check(candidates.isNotEmpty()) {
        "SurfaceFlinger 未列出本应用的 SurfaceView 图层：$SURFACE_VIEW_LAYER_PREFIX"
    }
    for (candidate in candidates) {
        if (readLayerFrameStats(device, candidate) != null) return candidate
    }
    error("本应用的 SurfaceView 图层都没有内容帧记录：$candidates")
}

/** 清空图层的帧历史：场景开始前调用，否则读到的会含上一个场景的残留帧。 */
internal fun clearLayerFrameHistory(device: UiDevice, layerName: String) {
    require(layerName.none { it.isWhitespace() }) { "图层名不得含空白：$layerName" }
    device.executeShellCommand("dumpsys SurfaceFlinger --latency-clear $layerName")
}

/** 读出指定图层的帧记录统计；图层未匹配或没有内容帧记录时返回 null。 */
internal fun readLayerFrameStats(device: UiDevice, layerName: String): LayerFrameStats? {
    require(layerName.none { it.isWhitespace() }) { "图层名不得含空白：$layerName" }
    return parseLayerFrameStats(device.executeShellCommand("dumpsys SurfaceFlinger --latency $layerName"))
}

/** 读出设备上的图层名清单。 */
private fun listLayerNames(device: UiDevice): List<String> = device.executeShellCommand("dumpsys SurfaceFlinger --list")
    .lineSequence()
    .map { layerNameOf(it) }
    .filter { it.isNotEmpty() }
    .toList()

/**
 * 从 `--list` 的一行里取出图层名。
 *
 * Android 15 起每行是 `RequestedLayerState{<名字> <键>=<值> ...}`，名字本身可能含
 * 空格（如 `Background for SurfaceView[...]`），故按「首个键值对之前的部分」取名字；
 * 更早的版本整行就是名字。
 */
private fun layerNameOf(listEntry: String): String = listEntry.trim()
    .removePrefix("RequestedLayerState{")
    .removeSuffix("}")
    .split(WHITESPACE)
    .takeWhile { !it.contains('=') }
    .joinToString(" ")
