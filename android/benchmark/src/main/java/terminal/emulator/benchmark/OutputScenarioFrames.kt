package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.UiDevice

/**
 * 输出类场景的帧面取数结果：把持续输出 / 突发输出 / 宽字符行共用的呈现面取数与判据
 * 收到一处，避免每个场景重复写一遍取数与断言。
 */
internal data class OutputScenarioFrames(
    val presentedFrames: Int,
    val presentedWindows: Int,
    val presentationGapMillis: Long,
    val layerStats: LayerFrameStats,
    val timelineContentFrames: Int,
    val attributedPresentTimesMillis: List<Double>,
    val layerPresentTimesMillis: List<Double>,
    val windowFramesCount: Int,
)

/**
 * 采集输出类场景的帧面：呈现汇总、逐图层通道、显示级帧时间线三通道，按属主归属到被测应用。
 *
 * [summaries] 是调用方已经等停摆后的呈现汇总；[loadStartMillis] 是负载开始的 epoch 毫秒；
 * [loadStartElapsedMillis] / [timelineReadElapsedMillis] 是单调时钟域的起止时刻。
 */
internal fun collectOutputScenarioFrames(
    device: UiDevice,
    packageName: String,
    contentLayer: String,
    loadStartMillis: Long,
    loadStartElapsedMillis: Long,
    timelineReadElapsedMillis: Long,
    summaries: List<PresentedFrameSummary>,
): OutputScenarioFrames {
    val timeline = readFrameTimeline(device)
    val layerStats = checkNotNull(readLayerFrameStats(device, contentLayer)) {
        "系统侧逐图层通道没有读到终端内容帧：图层 $contentLayer"
    }
    val layerPresentTimesMillis = layerStats.presentTimesNanos.map { it / 1_000_000.0 }
    val baseOffsetMillis = checkNotNull(
        timelineBaseOffsetMillis(
            timeline.layerPresentTimesMillis(contentLayer),
            layerPresentTimesMillis,
        ),
    ) {
        "显示级帧时间线与逐图层通道没有共同的呈现时刻：换算不出基准"
    }
    val windowFrames = timeline.withinWindow(
        loadStartElapsedMillis.toDouble(),
        timelineReadElapsedMillis.toDouble(),
        baseOffsetMillis,
    )
    val attributedContentFrames = windowFrames
        .layersOwnedBy(targetProcessId(device, packageName))
        .filter { it.name == contentLayer }
    val attributedPresentTimesMillis = attributedContentFrames
        .mapNotNull { it.presentTimeMillis }
        .map { it + baseOffsetMillis }
    val presentationGapMillis = summaries.zipWithNext { previous, current ->
        current.epochMillis - previous.epochMillis
    }.maxOrNull() ?: 0L
    val presentedFrames = summaries.sumOf { it.frames }
    return OutputScenarioFrames(
        presentedFrames = presentedFrames,
        presentedWindows = summaries.size,
        presentationGapMillis = presentationGapMillis,
        layerStats = layerStats,
        timelineContentFrames = attributedContentFrames.size,
        attributedPresentTimesMillis = attributedPresentTimesMillis,
        layerPresentTimesMillis = layerPresentTimesMillis,
        windowFramesCount = windowFrames.size,
    )
}

/**
 * 输出类场景帧面的六项判据：出帧、窗口数、停摆、间隔 p95、槽位占比、两通道一致性。
 */
internal fun assertOutputScenarioFrames(
    frames: OutputScenarioFrames,
    minPresentedWindows: Int,
    maxPresentationGapMillis: Long,
    maxLayerIntervalP95Millis: Long,
    minLayerSlotRatio: Double,
) {
    check(frames.presentedFrames > 0) {
        "输出期间没有呈现任何帧：终端没有画出负载产出的字节流"
    }
    check(frames.presentedWindows >= minPresentedWindows) {
        "输出只在 ${frames.presentedWindows} 个呈现窗口里出帧，低于下限 $minPresentedWindows：输出还在跑、终端不画了"
    }
    check(frames.presentationGapMillis <= maxPresentationGapMillis) {
        "呈现最长停摆 ${frames.presentationGapMillis}ms，超过上限 ${maxPresentationGapMillis}ms"
    }
    check(frames.layerStats.p95IntervalMillis <= maxLayerIntervalP95Millis) {
        "终端内容层呈现间隔 p95 ${frames.layerStats.p95IntervalMillis}ms 超过上限 ${maxLayerIntervalP95Millis}ms"
    }
    check(frames.layerStats.presentationSlotRatio >= minLayerSlotRatio) {
        "终端内容层只占用了 ${frames.layerStats.presentationSlotRatio} 的刷新槽位，低于下限 $minLayerSlotRatio"
    }
    check(frames.windowFramesCount > 0) {
        "显示级帧时间线在输出时间窗内没有帧"
    }
    check(frames.timelineContentFrames > 0) {
        "显示级帧时间线没有把终端内容层归属到被测应用"
    }
    check(samePresentEvents(frames.attributedPresentTimesMillis, frames.layerPresentTimesMillis)) {
        "显示级帧时间线与逐图层通道记录的呈现时刻对不上"
    }
}
