package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.math.max

/**
 * 持续输出场景 S5：终端持续收到字节流时，量吞吐、呈现节奏与内存峰值。
 *
 * 负载是设备侧 shell 自己产的字节流（见 [SustainedOutputLoad]）：固定批数、每批
 * 固定行数、批间固定间隔，故输出在整个窗口里持续到达，而不是一次性灌进来。每批
 * 结束打一个完成标记，批次是否真的跑完由此判定——呈现帧数不为零也证明不了字节流
 * 完整。
 *
 * 判据分三面：
 *
 * - 呈现面：原生呈现汇总的窗口数与最长停摆间隔（拦「输出还在跑、终端不画了」），
 *   系统侧逐图层的呈现间隔 p95 与刷新槽位比例（拦「画得越来越慢」）。
 * - 资源面：被测进程的 CPU 时间与内存峰值（拦「越跑越费」）。
 * - 流量面：吞吐下限（拦「字节流卡住」）。
 *
 * 内存峰值只能在负载进行中取：负载结束后内存可能回落。故等完成标记的那段轮询顺带
 * 按固定间隔采内存。
 */
@RunWith(AndroidJUnit4::class)
class ContinuousOutputBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun sustainedOutput() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("sustainedOutput")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        // 就绪信号是会话的首个输出，那条记录写在会话启动之后，故取应用启动前的时刻。
        var sessionStartMillis = 0L
        lateinit var contentLayer: String
        measureScenario(
            device = device,
            label = "sustainedOutput",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S5"),
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                recoverEmulator(device)
                // 本场景的呈现汇总是 debug 级日志，进程等级不够就读不到帧。
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                // shell 就绪是负载的前提，不是被测量的行为：它可能耗上几十秒，
                // 留在测量窗口里会把场景的 wall-clock 上限吃掉。
                contentLayer = awaitTerminalContentFrames(device).first
                awaitShellReady(device, sessionStartMillis)
            },
        ) { _ ->
            val load = SustainedOutputLoad(
                batchCount = SUSTAINED_OUTPUT_BATCH_COUNT,
                linesPerBatch = SUSTAINED_OUTPUT_LINES_PER_BATCH,
                pauseSeconds = SUSTAINED_OUTPUT_BATCH_PAUSE_SECONDS,
            )
            // 负载命令先敲进去，回车才开跑：命令回显的渲染不属于输出负载。
            device.executeShellCommand("input text ${load.inputTextArgument()}")
            clearLayerFrameHistory(device, contentLayer)
            resetFrameStats(device)
            val cpuBefore = readProcessCpuTime(device, TARGET_PACKAGE)
            val baselineMemory = readDeviceMemory(device, TARGET_PACKAGE)
            val loadStartMillis = System.currentTimeMillis()
            val loadStartElapsedMillis = SystemClock.elapsedRealtime()
            device.executeShellCommand("input keyevent 66")
            val outcome = awaitLoadCompletion(device, loadStartMillis, load.batchCount)
            if (outcome.markers.size < load.batchCount) {
                // 负载没跑完就别让它继续跑：强停被测应用，不在失败场景上继续耗设备。
                forceStop(device, TARGET_PACKAGE)
            }
            val summaries = awaitPresentedSettle(device, loadStartMillis)
            val cpuAfter = readProcessCpuTime(device, TARGET_PACKAGE)
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
                loadStartElapsedMillis.toDouble(),
                timelineReadElapsedMillis.toDouble(),
                baseOffsetMillis,
            )
            val attributedContentFrames = windowFrames
                .layersOwnedBy(targetProcessId(device))
                .filter { it.name == contentLayer }
            val attributedPresentTimesMillis = attributedContentFrames
                .mapNotNull { it.presentTimeMillis }
                .map { it + baseOffsetMillis }
            val deliveredBytes = outcome.markers.size * load.bytesPerBatch()
            val loadDurationMillis = outcome.markers.lastOrNull()?.epochMillis?.minus(loadStartMillis) ?: 0L
            val throughputBytesPerSecond = if (loadDurationMillis > 0) {
                deliveredBytes * 1000.0 / loadDurationMillis
            } else {
                0.0
            }
            val processCpuMillis = cpuAfter.totalMillis - cpuBefore.totalMillis
            val peakPssKilobytes = max(outcome.peakPssKilobytes, baselineMemory.pssKilobytes.toLong())
            val peakRssKilobytes = max(outcome.peakRssKilobytes, baselineMemory.rssKilobytes.toLong())
            val presentationGapMillis = summaries.zipWithNext { previous, current ->
                current.epochMillis - previous.epochMillis
            }.maxOrNull() ?: 0L
            val presentedFrames = summaries.sumOf { it.frames }
            printMeasured("sustainedOutput", loadDurationMillis, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("sustainedOutput", loadDurationMillis) +
                    " deliveredBytes=$deliveredBytes" +
                    " throughput=${throughputBytesPerSecond}B/s" +
                    " presentedFrames=$presentedFrames" +
                    " presentedWindows=${summaries.size}" +
                    " maxPresentationGap=${presentationGapMillis}ms" +
                    " layerFrames=${layerStats.contentFrameCount}" +
                    " layerIntervalP50=${layerStats.medianIntervalMillis}ms" +
                    " layerIntervalP95=${layerStats.p95IntervalMillis}ms" +
                    " layerIntervalMax=${layerStats.maxIntervalMillis}ms" +
                    " layerSlotRatio=${layerStats.presentationSlotRatio}" +
                    " timelineFrames=${windowFrames.size}" +
                    " timelineContentFrames=${attributedContentFrames.size}" +
                    " processCpu=${processCpuMillis}ms" +
                    " baselinePss=${baselineMemory.pssKilobytes}KB" +
                    " peakPss=${peakPssKilobytes}KB" +
                    " pssGrowth=${peakPssKilobytes - baselineMemory.pssKilobytes}KB" +
                    " peakRss=${peakRssKilobytes}KB",
            )
            check(outcome.markers.size == load.batchCount) {
                "负载没跑完：只收到 ${outcome.markers.size} 个完成标记（应 ${load.batchCount} 个）"
            }
            check(presentedFrames > 0) {
                "持续输出期间没有呈现任何帧：终端没有画出负载产出的字节流"
            }
            check(summaries.size >= MIN_SUSTAINED_OUTPUT_PRESENTED_WINDOWS) {
                "持续输出只在 ${summaries.size} 个呈现窗口里出帧，低于下限 " +
                    "${MIN_SUSTAINED_OUTPUT_PRESENTED_WINDOWS}：输出还在跑、终端不画了"
            }
            check(presentationGapMillis <= MAX_SUSTAINED_OUTPUT_PRESENTATION_GAP_MILLIS) {
                "呈现最长停摆 ${presentationGapMillis}ms，超过上限 " +
                    "${MAX_SUSTAINED_OUTPUT_PRESENTATION_GAP_MILLIS}ms"
            }
            check(layerStats.p95IntervalMillis <= MAX_SUSTAINED_OUTPUT_LAYER_INTERVAL_P95_MILLIS) {
                "终端内容层呈现间隔 p95 ${layerStats.p95IntervalMillis}ms 超过上限 " +
                    "${MAX_SUSTAINED_OUTPUT_LAYER_INTERVAL_P95_MILLIS}ms"
            }
            check(layerStats.presentationSlotRatio >= MIN_SUSTAINED_OUTPUT_LAYER_SLOT_UTILIZATION) {
                "终端内容层只占用了 ${layerStats.presentationSlotRatio} 的刷新槽位，低于下限 " +
                    "${MIN_SUSTAINED_OUTPUT_LAYER_SLOT_UTILIZATION}"
            }
            check(windowFrames.isNotEmpty()) {
                "显示级帧时间线在持续输出时间窗内没有帧"
            }
            check(attributedContentFrames.isNotEmpty()) {
                "显示级帧时间线没有把终端内容层归属到被测应用"
            }
            check(samePresentEvents(attributedPresentTimesMillis, layerPresentTimesMillis)) {
                "显示级帧时间线与逐图层通道记录的呈现时刻对不上"
            }
            check(peakPssKilobytes - baselineMemory.pssKilobytes <= MAX_SUSTAINED_OUTPUT_PSS_GROWTH_KILOBYTES) {
                "持续输出期间 PSS 增长 " +
                    "${peakPssKilobytes - baselineMemory.pssKilobytes}KB 超过上限 " +
                    "${MAX_SUSTAINED_OUTPUT_PSS_GROWTH_KILOBYTES}KB：内存无界"
            }
            check(processCpuMillis <= MAX_SUSTAINED_OUTPUT_PROCESS_CPU_MILLIS) {
                "持续输出期间进程 CPU 时间 ${processCpuMillis}ms 超过上限 " +
                    "${MAX_SUSTAINED_OUTPUT_PROCESS_CPU_MILLIS}ms"
            }
            check(throughputBytesPerSecond >= MIN_SUSTAINED_OUTPUT_THROUGHPUT_BYTES_PER_SECOND) {
                "持续输出吞吐 ${throughputBytesPerSecond}B/s 低于下限 " +
                    "${MIN_SUSTAINED_OUTPUT_THROUGHPUT_BYTES_PER_SECOND}B/s"
            }
            mapOf(
                "delivered_bytes" to deliveredBytes.toDouble(),
                "throughput_bytes_per_second" to throughputBytesPerSecond,
                "presented_frames" to presentedFrames.toDouble(),
                "presented_windows" to summaries.size.toDouble(),
                "max_presentation_gap_millis" to presentationGapMillis.toDouble(),
                "layer_content_frames" to layerStats.contentFrameCount.toDouble(),
                "layer_interval_p95_millis" to layerStats.p95IntervalMillis.toDouble(),
                "layer_slot_ratio" to layerStats.presentationSlotRatio,
                "timeline_content_frames" to attributedContentFrames.size.toDouble(),
                "process_cpu_millis" to processCpuMillis.toDouble(),
                "peak_pss_kilobytes" to peakPssKilobytes.toDouble(),
                "peak_rss_kilobytes" to peakRssKilobytes.toDouble(),
            )
        }
    }

    /**
     * 等负载跑完：每批一个完成标记，标记齐了即负载结束。
     *
     * 顺带按固定间隔采内存：峰值只能在这段里取——负载结束后内存可能回落。
     */
    private fun awaitLoadCompletion(device: UiDevice, loadStartMillis: Long, batchCount: Int): LoadOutcome {
        val deadline = SystemClock.elapsedRealtime() + SUSTAINED_OUTPUT_TIMEOUT_MILLIS
        var markers = emptyList<LogcatRecord>()
        var peakPssKilobytes = 0L
        var peakRssKilobytes = 0L
        var nextMemorySampleElapsedMillis = 0L
        while (SystemClock.elapsedRealtime() < deadline) {
            markers = readLogcatSince(device, loadStartMillis, SUSTAINED_OUTPUT_MARKER_PATTERN)
            if (markers.size >= batchCount) break
            val now = SystemClock.elapsedRealtime()
            if (now >= nextMemorySampleElapsedMillis) {
                val memory = readDeviceMemory(device, TARGET_PACKAGE)
                peakPssKilobytes = max(peakPssKilobytes, memory.pssKilobytes.toLong())
                peakRssKilobytes = max(peakRssKilobytes, memory.rssKilobytes.toLong())
                nextMemorySampleElapsedMillis = now + SUSTAINED_OUTPUT_MEMORY_SAMPLE_INTERVAL_MILLIS
            }
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        return LoadOutcome(markers, peakPssKilobytes, peakRssKilobytes)
    }

    /** 负载跑完时的取证结果：完成标记与采到的内存峰值。 */
    private data class LoadOutcome(
        /** 完成标记（每批一条）。 */
        val markers: List<LogcatRecord>,
        /** 负载进行中采到的 PSS 峰值（千字节）。 */
        val peakPssKilobytes: Long,
        /** 负载进行中采到的 RSS 峰值（千字节）。 */
        val peakRssKilobytes: Long,
    )
}
