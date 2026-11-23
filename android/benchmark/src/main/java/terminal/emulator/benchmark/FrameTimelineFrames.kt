package terminal.emulator.benchmark

import androidx.test.uiautomator.UiDevice
import kotlin.math.abs
import kotlin.math.roundToLong

/*
 * 系统侧显示级帧时间线（`dumpsys SurfaceFlinger --frametimeline -all`）。
 *
 * 该面是唯一同时具备「同口径」与「属主归属」的通道：逐 display frame 给出呈现时刻，
 * 每个 display frame 下挂着该帧涉及的逐图层条目（含 `Owner Pid`），故不预知图层名
 * 也能把帧归属到某个应用的内容图层。
 *
 * 它的时刻不是稳定时钟：dump 的时刻以「保留窗口里最旧的一帧」为基准（AOSP
 * `FrameTimeline::dumpAll` 取 `mDisplayFrames[0]->getBaseTime()`），窗口滑动时基准
 * 前移。实测（2026-11-21 本机 AVD，API 35 模拟器）终端持续输出期间相隔 8 秒的两次
 * dump，最新一帧的相对时刻都是约 1.05 秒（保留窗口的跨度），而不是 8 秒后。取场景
 * 时间窗须先换算基准，见 [timelineBaseOffsetMillis]。
 */

/** 帧时间线里一个图层的条目：某个 display frame 中该图层的一帧。 */
internal data class TimelineLayerFrame(
    /** 图层名（含实例后缀），与 `dumpsys SurfaceFlinger --list` 的名字同形。 */
    val name: String,
    /** 属主进程号：跨应用归属到内容图层靠它。 */
    val ownerPid: Int,
    /** 图层号。 */
    val layerId: Int,
    /** 是否带缓冲：带缓冲的条目才是内容帧。 */
    val isBuffer: Boolean,
    /** 呈现状态（`Presented` / `Dropped` 等）。 */
    val presentState: String,
    /** 相对 dump 基准的呈现时刻（毫秒）；该帧未呈现时为 null。 */
    val presentTimeMillis: Double?,
)

/** 帧时间线里的一个 display frame：合成器的一帧及其涉及的图层条目。 */
internal data class TimelineDisplayFrame(
    /** display frame 在 dump 里的序号（0 是保留窗口里最旧的一帧）。 */
    val index: Int,
    /** 相对 dump 基准的呈现时刻（毫秒）；该帧还没呈现时为 null。 */
    val presentTimeMillis: Double?,
    /** 该帧的 jank 分类（`None` / `Prediction Error` 等）。 */
    val jankType: String,
    /** 该帧涉及的图层条目。 */
    val layers: List<TimelineLayerFrame>,
)

/** 基准偏移归一的档宽（毫秒）：呈现时刻的抖动远小于它，同批事件必落同一档。 */
private const val BASE_OFFSET_BUCKET_MILLIS = 0.1

/** 两条通道判定「同一批呈现事件」的容差（毫秒）。 */
private const val SAME_PRESENT_TOLERANCE_MILLIS = 1.0

/** `Display Frame 12` 与可选的 jank 标记 `[*]`。 */
private val DISPLAY_FRAME_PATTERN = Regex("""Display Frame (\d+)""")

/**
 * 解析 `--frametimeline -all` 的输出。
 *
 * 输出按 display frame 分块：块首是 `Display Frame N`，块内先是该帧自身的字段与
 * 期望/实际时刻表，其后是各图层条目，每个条目以 `Layer - ` 开头并带自己的时刻表。
 * 帧还没呈现时实际时刻表里的呈现时刻是 `N/A`，记为 null。
 */
internal fun parseFrameTimeline(output: String): List<TimelineDisplayFrame> {
    val frames = mutableListOf<TimelineDisplayFrame>()
    var currentIndex: Int? = null
    var currentLines = mutableListOf<String>()
    for (line in output.lineSequence()) {
        val match = DISPLAY_FRAME_PATTERN.find(line)
        if (match != null && line.startsWith("Display Frame ")) {
            currentIndex?.let { index -> frames += parseDisplayFrame(index, currentLines) }
            currentIndex = match.groupValues[1].toInt()
            currentLines = mutableListOf()
        } else if (currentIndex != null) {
            currentLines += line
        }
    }
    currentIndex?.let { index -> frames += parseDisplayFrame(index, currentLines) }
    return frames
}

/** 解析一个 display frame 块：帧自身字段与其中的图层条目。 */
private fun parseDisplayFrame(index: Int, lines: List<String>): TimelineDisplayFrame {
    val layerHeaderIndexes = lines.withIndex()
        .filter { (_, line) -> line.trim().startsWith("Layer - ") }
        .map { (lineIndex, _) -> lineIndex }
    val frameLines = if (layerHeaderIndexes.isEmpty()) lines else lines.subList(0, layerHeaderIndexes.first())
    var presentTimeMillis: Double? = null
    var jankType = ""
    for (line in frameLines) {
        val trimmed = line.trim()
        when {
            trimmed.startsWith("Jank Type") -> jankType = trimmed.substringAfter(':').trim()
            trimmed.startsWith("Actual") -> presentTimeMillis = presentTimeMillisOf(trimmed)
        }
    }
    val layers = mutableListOf<TimelineLayerFrame>()
    layerHeaderIndexes.forEachIndexed { position, headerIndex ->
        val endIndex = layerHeaderIndexes.getOrElse(position + 1) { lines.size }
        layers += parseLayerFrame(lines.subList(headerIndex, endIndex))
    }
    return TimelineDisplayFrame(index, presentTimeMillis, jankType, layers)
}

/** 解析一个图层条目块：图层名、属主、图层号与呈现时刻。 */
private fun parseLayerFrame(lines: List<String>): TimelineLayerFrame {
    var name = ""
    var ownerPid = 0
    var layerId = 0
    var isBuffer = false
    var presentState = ""
    var presentTimeMillis: Double? = null
    for (line in lines) {
        val trimmed = line.trim()
        when {
            trimmed.startsWith("Layer - ") -> name = layerNameOf(trimmed)
            trimmed.startsWith("Owner Pid") -> ownerPid = trimmed.substringAfter(':').trim().toIntOrNull() ?: 0
            trimmed.startsWith("Layer ID") -> layerId = trimmed.substringAfter(':').trim().toIntOrNull() ?: 0
            trimmed.startsWith("Is Buffer?") -> isBuffer = trimmed.substringAfter(':').trim() == "1"
            trimmed.startsWith("Present State") -> presentState = trimmed.substringAfter(':').trim()
            trimmed.startsWith("Actual") -> presentTimeMillis = presentTimeMillisOf(trimmed)
        }
    }
    return TimelineLayerFrame(name, ownerPid, layerId, isBuffer, presentState, presentTimeMillis)
}

/**
 * 图层名：去掉 `Layer - ` 前缀、`TX - ` 事务前缀与 jank 标记。
 *
 * 标记是 dumper 给「jank 类型不为 None」的帧加的（`[*]`），不属于图层名。
 */
private fun layerNameOf(header: String): String = header.removePrefix("Layer - ")
    .removeSuffix(" [*]")
    .trim()
    .removePrefix("TX - ")
    .trim()

/** 时刻表里 `Actual` 行的呈现时刻（毫秒）；未呈现时是 `N/A`，记为 null。 */
private fun presentTimeMillisOf(row: String): Double? {
    val columns = row.split('|')
    if (columns.size != 4) return null
    return columns[3].trim().toDoubleOrNull()
}

/** 读出设备上的显示级帧时间线。 */
internal fun readFrameTimeline(device: UiDevice): List<TimelineDisplayFrame> =
    parseFrameTimeline(device.executeShellCommand("dumpsys SurfaceFlinger --frametimeline -all"))

/** 被测应用的进程号：帧时间线按属主归属到内容图层时用它。 */
internal fun targetProcessId(device: UiDevice, packageName: String = TARGET_PACKAGE): Int {
    val output = device.executeShellCommand("pidof $packageName").trim()
    val processId = output.split(WHITESPACE).firstOrNull()?.toIntOrNull()
    return checkNotNull(processId) { "取不到被测应用的进程号：pidof $packageName 返回「$output」" }
}

/** 某图层在帧时间线里的呈现时刻（相对 dump 基准，毫秒），按时间升序。 */
internal fun List<TimelineDisplayFrame>.layerPresentTimesMillis(layerName: String): List<Double> = flatMap { frame ->
    frame.layers.filter { it.name == layerName }.mapNotNull { it.presentTimeMillis }
}

/** 归属到某进程的图层条目。 */
internal fun List<TimelineDisplayFrame>.layersOwnedBy(ownerPid: Int): List<TimelineLayerFrame> =
    flatMap { it.layers }.filter { it.ownerPid == ownerPid }

/**
 * 帧时间线到绝对时刻的基准偏移（毫秒）。
 *
 * 帧时间线的相对时刻以保留窗口里最旧的一帧为基准，基准随窗口滑动而前移；逐图层的
 * `--latency` 通道给的是绝对纳秒时刻（CLOCK_MONOTONIC，与 `elapsedRealtime` 同域）。
 * 两条通道记录的是同一批呈现事件，故按呈现时刻序列对齐即可换算出基准：对每一对时刻
 * 求差，落在同一档（[BASE_OFFSET_BUCKET_MILLIS]）的差值即同一批事件，取众数档的中位数
 * 即基准偏移。共同事件不足两个时无从对齐，返回 null。
 */
internal fun timelineBaseOffsetMillis(
    timelinePresentTimesMillis: List<Double>,
    latencyPresentTimesMillis: List<Double>,
): Double? {
    if (timelinePresentTimesMillis.isEmpty() || latencyPresentTimesMillis.isEmpty()) return null
    val differences = mutableListOf<Double>()
    for (timelineTime in timelinePresentTimesMillis) {
        for (latencyTime in latencyPresentTimesMillis) {
            differences += latencyTime - timelineTime
        }
    }
    val buckets = differences.groupBy { (it / BASE_OFFSET_BUCKET_MILLIS).roundToLong() }
    val bestBucket = buckets.maxByOrNull { it.value.size } ?: return null
    if (bestBucket.value.size < 2) return null
    val sorted = bestBucket.value.sorted()
    return sorted[sorted.size / 2]
}

/**
 * 场景时间窗内的 display frame（绝对时刻，毫秒）。
 *
 * 窗口是场景的起止时刻（`elapsedRealtime` 域），[baseOffsetMillis] 是帧时间线相对
 * 时刻到绝对时刻的偏移。
 */
internal fun List<TimelineDisplayFrame>.withinWindow(
    startMillis: Double,
    endMillis: Double,
    baseOffsetMillis: Double,
): List<TimelineDisplayFrame> = filter { frame ->
    val presentTimeMillis = frame.presentTimeMillis ?: return@filter false
    val absoluteMillis = presentTimeMillis + baseOffsetMillis
    absoluteMillis >= startMillis && absoluteMillis <= endMillis
}

/**
 * 两条通道是否记录了同一批呈现事件：逐个比对呈现时刻。
 *
 * 帧时间线与逐图层通道看的是同一批物理呈现，故归属到同一图层的呈现时刻应当一一
 * 对得上；差值超过 [SAME_PRESENT_TOLERANCE_MILLIS] 即两边的取数出了偏差。
 */
internal fun samePresentEvents(
    timelinePresentTimesMillis: List<Double>,
    latencyPresentTimesMillis: List<Double>,
    toleranceMillis: Double = SAME_PRESENT_TOLERANCE_MILLIS,
): Boolean {
    if (timelinePresentTimesMillis.isEmpty()) return false
    return timelinePresentTimesMillis.all { timelineTime ->
        latencyPresentTimesMillis.any { latencyTime -> abs(latencyTime - timelineTime) <= toleranceMillis }
    }
}
