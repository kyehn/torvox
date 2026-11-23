package terminal.emulator.benchmark

import android.graphics.Rect
import android.os.SystemClock
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val SCROLL_ITERATION_COUNT = 3

/** 拖动手势的时长：连续拖动才持续出帧，瞬时滑动只有一帧 fling。 */
private const val DRAG_DURATION_MILLIS = 1_500L

/** 填满回滚区的命令：500 行足够把视口顶出历史区。 */
private const val SCROLLBACK_COMMAND = "seq%s500"

/**
 * 滚动的帧级宏基准：在回滚区被填满的状态下拖向历史、再拖回底部各一次。
 *
 * TESTING 性能条款把「上滑/下滑正确移动终端，无卡顿」列为覆盖范围，app 侧已有
 * 行为量化测试钉正确性，这里补宏基准侧的流畅度与耗时。内容生成走真实输入链路：
 * `input text` 把 `seq 500` 敲进 shell（应用首启自动创建默认会话），随后拖动
 * 终端内容本身。
 *
 * 帧的来源是原生渲染器的呈现汇总而非 `dumpsys gfxinfo`：终端内容由原生渲染线程
 * 直绘 SurfaceView，不经过主线程 `ViewRootImpl` 遍历，gfxinfo 记不到这些帧
 * （实测滚动全程 0 帧，见 [PresentedFrames]）。`FrameTimingMetric` 仍留在指标列表
 * 里，它记的是 Compose 层的帧。
 *
 * 滚动窗口另有系统侧逐图层通道的独立证据：终端内容层在滚动前清空帧历史，滚动后
 * 读回逐帧呈现时刻，确认内容确实呈现到了合成器（见 [SurfaceLayerFrames]）。
 */
@RunWith(AndroidJUnit4::class)
class ScrollPerformanceBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun scrollWithFullScrollback() {
        val store = openMeasurementStore("scroll")
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(FrameTimingMetric()),
            iterations = SCROLL_ITERATION_COUNT,
            setupBlock = {
                grantNotificationPermission(device)
                recoverEmulator(device)
            },
            measureBlock = {
                measureScenario(device, "scroll", store, iteration, scenarioById("S7")) { _, _ ->
                    val startedMillis = System.currentTimeMillis()
                    startActivityAndWait(benchmarkLaunchIntent())
                    val terminal =
                        requireElement(device, TERMINAL_SCREEN_RESOURCE_ID, "终端界面")
                    awaitShellReady(device, startedMillis)
                    fillScrollback(device)
                    val contentLayer = findTerminalContentLayer(device)
                    clearLayerFrameHistory(device, contentLayer)
                    val scrollStartMillis = System.currentTimeMillis()
                    val scrollStartElapsedMillis = SystemClock.elapsedRealtime()
                    val elapsed = elapsedMillis {
                        dragTerminal(device, terminal.visibleBounds, towardHistory = true)
                        dragTerminal(device, terminal.visibleBounds, towardHistory = false)
                    }
                    val summaries = awaitPresentedSettle(device, scrollStartMillis)
                    val presentedFrames = summaries.sumOf { it.frames }
                    // 显示级帧时间线是同一批呈现的第二个系统侧观测面：它按属主归属到应用，
                    // 不依赖图层名。先读它再读逐图层通道——后者保留的帧更多，先读的那份
                    // 不会出现「有帧而另一条通道没有」的单向缺口。
                    val timeline = readFrameTimeline(device)
                    val timelineReadElapsedMillis = SystemClock.elapsedRealtime()
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
                        scrollStartElapsedMillis.toDouble(),
                        timelineReadElapsedMillis.toDouble(),
                        baseOffsetMillis,
                    )
                    val attributedContentFrames = windowFrames
                        .layersOwnedBy(targetProcessId(device))
                        .filter { it.name == contentLayer }
                    val attributedPresentTimesMillis = attributedContentFrames
                        .mapNotNull { it.presentTimeMillis }
                        .map { it + baseOffsetMillis }
                    // 实测值先于断言输出：断言失败时构建日志里仍留得下这轮的数字，
                    // 否则门禁只报「低于下限」，排障还得重跑一遍。
                    emitMeasured(
                        measuredLinePrefix("scroll", elapsed) +
                            " presentedFrames=$presentedFrames presentedWindows=${summaries.size}" +
                            " layerFrames=${layerStats.contentFrameCount}" +
                            " layerMissed=${layerStats.missedFrameCount}" +
                            " layerRate=${layerStats.presentationRatePerSecond}fps" +
                            " layerSlotRatio=${layerStats.presentationSlotRatio}" +
                            " layerIntervalP50=${layerStats.medianIntervalMillis}ms" +
                            " layerIntervalP90=${layerStats.p90IntervalMillis}ms" +
                            " layerIntervalMax=${layerStats.maxIntervalMillis}ms" +
                            " timelineFrames=${windowFrames.size}" +
                            " timelineContentFrames=${attributedContentFrames.size}" +
                            " timelineBaseOffset=${baseOffsetMillis}ms",
                    )
                    check(summaries.isNotEmpty()) {
                        "滚动未产生呈现帧：原生渲染器在滚动窗口内没有汇报任何呈现"
                    }
                    check(summaries.size >= MIN_SCROLL_PRESENTED_WINDOWS) {
                        "滚动只在 ${summaries.size} 个窗口里出帧，低于下限 $MIN_SCROLL_PRESENTED_WINDOWS：" +
                            "滚动中途停摆"
                    }
                    check(presentedFrames >= MIN_SCROLL_PRESENTED_FRAMES) {
                        "滚动呈现帧数 $presentedFrames 低于下限 $MIN_SCROLL_PRESENTED_FRAMES"
                    }
                    check(elapsed <= SCROLL_BUDGET_MILLIS) {
                        "上滑下滑耗时 ${elapsed}ms 超过预算 ${SCROLL_BUDGET_MILLIS}ms"
                    }
                    check(layerStats.presentationSlotRatio >= MIN_SCROLL_LAYER_SLOT_UTILIZATION) {
                        "终端内容层在滚动窗口内只占用了 ${layerStats.presentationSlotRatio} 的刷新槽位，" +
                            "低于下限 $MIN_SCROLL_LAYER_SLOT_UTILIZATION"
                    }
                    check(windowFrames.isNotEmpty()) {
                        "显示级帧时间线在滚动时间窗内没有帧"
                    }
                    check(attributedContentFrames.isNotEmpty()) {
                        "显示级帧时间线没有把终端内容层归属到被测应用"
                    }
                    check(attributedContentFrames.size >= MIN_SCROLL_TIMELINE_CONTENT_FRAMES) {
                        "显示级帧时间线只归属到 ${attributedContentFrames.size} 帧终端内容，" +
                            "低于下限 $MIN_SCROLL_TIMELINE_CONTENT_FRAMES"
                    }
                    check(samePresentEvents(attributedPresentTimesMillis, layerPresentTimesMillis)) {
                        "显示级帧时间线与逐图层通道记录的呈现时刻对不上"
                    }
                    emptyMap()
                }
            },
        )
    }

    /**
     * 敲入 `seq 500` 并等输出渲染完毕。
     *
     * `input text` 走硬件键盘事件通道，不经过输入法，终端像收到物理键盘一样把
     * 字符送进 shell（`%s` 是 `input text` 的空格转义）；回车执行后 500 行输出
     * 把回滚区填满。判定「渲染完毕」看原生侧的呈现汇总：输出期间持续出帧，输出
     * 结束后不再有新的汇总行。
     */
    private fun fillScrollback(device: UiDevice) {
        val commandMillis = System.currentTimeMillis()
        device.executeShellCommand("input text $SCROLLBACK_COMMAND")
        device.executeShellCommand("input keyevent 66")
        awaitPresentedSettle(device, commandMillis)
    }

    /**
     * 一次拖动手势：手指下移把视图拖向历史，上移拖回底部。
     *
     * 用 `input swipe` 的显式时长而非瞬时滑动：瞬时滑动只产生一帧 fling，测不出
     * 连续出帧的节奏；固定时长的拖动在设备之间可比。坐标取终端界面可见范围的
     * 上半部，避开底部的修饰键栏。
     */
    private fun dragTerminal(device: UiDevice, terminalBounds: Rect, towardHistory: Boolean) {
        val centerX = terminalBounds.centerX()
        val upperY = terminalBounds.top + (terminalBounds.height() * 0.2).toInt()
        val lowerY = terminalBounds.top + (terminalBounds.height() * 0.45).toInt()
        val startY = if (towardHistory) upperY else lowerY
        val endY = if (towardHistory) lowerY else upperY
        device.executeShellCommand(
            "input swipe $centerX $startY $centerX $endY $DRAG_DURATION_MILLIS",
        )
    }
}
