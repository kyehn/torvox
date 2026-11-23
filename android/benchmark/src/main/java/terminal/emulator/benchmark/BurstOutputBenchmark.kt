package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.math.max

/** 完成标记的匹配：正文形如 `__s6done`。 */
internal val BURST_OUTPUT_MARKER_PATTERN = Regex(BURST_OUTPUT_MARKER)

/**
 * 回滚行数的匹配：应用侧帧计时汇总里的 `scrollback=1972 rows`。
 *
 * 空白用 `.` 代替：该正则既作设备侧 `-e` 过滤词（不经 shell，按空白切词），又作
 * 本地复核的判据，两边必须同源。
 */
private val SCROLLBACK_ROWS_PATTERN = Regex("""scrollback=([0-9]+).rows""")

/**
 * 突发输出场景 S6：终端一次性收到大块字节流时，量消费时长、呈现节奏与内存峰值。
 *
 * 负载是设备侧 shell 自己产的整段字节流（见 [BurstOutputLoad]）：一次灌完，中间没有
 * 节流。流的末尾带两个哨兵，顺序即证据链——标题哨兵（OSC 2）出现在应用状态里证明
 * 终端把整段流解析到了末尾，完成标记（logcat）证明生产者把全部字节写进了 PTY。
 *
 * 判据分四面：
 *
 * - 内容面：标题哨兵出现在会话抽屉里（拦「流被截断、终端只吃到一半」）。
 * - 呈现面：原生呈现汇总的窗口数与最长停摆间隔、系统侧逐图层的呈现间隔 p95 与刷新
 *   槽位比例（拦「整段字节流把终端压死了」）。
 * - 资源面：被测进程的 CPU 时间与内存峰值（拦「越跑越费」）。
 * - 归因面：测量窗口内进程退出即判红（由 [measureScenario] 统一归因）。
 *
 * 内存峰值只能在负载进行中取：负载结束后内存可能回落。故等完成标记的那段轮询顺带
 * 按固定间隔采内存。
 */
@RunWith(AndroidJUnit4::class)
class BurstOutputBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun burstOutput() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("burstOutput")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        // 就绪信号是会话的首个输出，那条记录写在会话启动之后，故取应用启动前的时刻。
        var sessionStartMillis = 0L
        lateinit var contentLayer: String
        measureScenario(
            device = device,
            label = "burstOutput",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S6"),
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                // 通知权限弹窗会抢走焦点：应用被弹窗盖住时输入法不弹出，终端也拿不到
                // 输入焦点，场景会死在「输入法未弹出」而不是被测行为上。
                grantNotificationPermission(device)
                // 突发场景要一个干净会话：上一轮遗留的运行中程序会把敲键吃掉，负载
                // 根本没跑起来而界面照旧在画。
                recoverEmulator(device)
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                // shell 就绪是负载的前提，不是被测量的行为：它可能耗上几十秒，
                // 留在测量窗口里会把场景的 wall-clock 上限吃掉。
                contentLayer = awaitTerminalContentFrames(device).first
                awaitShellReady(device, sessionStartMillis)
                disablePromptTitleReport(device)
            },
        ) { _, _ ->
            val load = BurstOutputLoad(lineCount = BURST_OUTPUT_LINE_COUNT, canary = BURST_OUTPUT_CANARY)
            check(load.bytes() <= scenarioById("S6").outputByteLimit) {
                "突发负载 ${load.bytes()} 字节超过场景输出上限 " +
                    "${scenarioById("S6").outputByteLimit} 字节"
            }
            // 负载命令先敲进去，回车才开跑：命令回显的渲染不属于输出负载。
            device.executeShellCommand("input text ${load.inputTextArgument()}")
            clearLayerFrameHistory(device, contentLayer)
            resetFrameStats(device)
            val cpuBefore = readProcessCpuTime(device, TARGET_PACKAGE)
            val baselineMemory = readDeviceMemory(device, TARGET_PACKAGE)
            val loadStartMillis = System.currentTimeMillis()
            val loadStartElapsedMillis = SystemClock.elapsedRealtime()
            device.executeShellCommand("input keyevent 66")
            val outcome = awaitLoadCompletion(device, loadStartMillis)
            if (!outcome.completed) {
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
            val burstDurationMillis = outcome.completedAtMillis - loadStartMillis
            val processCpuMillis = cpuAfter.totalMillis - cpuBefore.totalMillis
            val peakPssKilobytes = max(outcome.peakPssKilobytes, baselineMemory.pssKilobytes.toLong())
            val peakRssKilobytes = max(outcome.peakRssKilobytes, baselineMemory.rssKilobytes.toLong())
            val presentationGapMillis = summaries.zipWithNext { previous, current ->
                current.epochMillis - previous.epochMillis
            }.maxOrNull() ?: 0L
            val presentedFrames = summaries.sumOf { it.frames }
            val frameStats = readFrameStats(device)
            // 标题哨兵在应用状态里显形即证明终端消费到了流末尾：读取会打开会话抽屉，
            // 故放在帧面取数之后，不让抽屉动画混进突发窗口的帧。
            val canaryVisible = awaitSessionTitleCanary(device, load.canary)
            printMeasured("burstOutput", burstDurationMillis, frameStats)
            emitMeasured(
                measuredLinePrefix("burstOutput", burstDurationMillis) +
                    " deliveredBytes=${load.bytes()}" +
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
                    " peakRss=${peakRssKilobytes}KB" +
                    " scrollbackRows=${outcome.scrollbackRows}" +
                    " canaryVisible=$canaryVisible",
            )
            check(outcome.completed) {
                "负载没跑完：${BURST_OUTPUT_TIMEOUT_MILLIS}ms 内没有收到完成标记"
            }
            check(canaryVisible) {
                "流末尾的标题哨兵没有出现在会话标题里：终端没有把整段字节流解析到末尾"
            }
            check(presentedFrames > 0) {
                "突发输出期间没有呈现任何帧：终端没有画出负载产出的字节流"
            }
            check(summaries.size >= MIN_BURST_OUTPUT_PRESENTED_WINDOWS) {
                "突发输出只在 ${summaries.size} 个呈现窗口里出帧，低于下限 " +
                    "${MIN_BURST_OUTPUT_PRESENTED_WINDOWS}：整段字节流把终端压死了"
            }
            check(presentationGapMillis <= MAX_BURST_OUTPUT_PRESENTATION_GAP_MILLIS) {
                "呈现最长停摆 ${presentationGapMillis}ms，超过上限 " +
                    "${MAX_BURST_OUTPUT_PRESENTATION_GAP_MILLIS}ms"
            }
            check(layerStats.p95IntervalMillis <= MAX_BURST_OUTPUT_LAYER_INTERVAL_P95_MILLIS) {
                "终端内容层呈现间隔 p95 ${layerStats.p95IntervalMillis}ms 超过上限 " +
                    "${MAX_BURST_OUTPUT_LAYER_INTERVAL_P95_MILLIS}ms"
            }
            check(layerStats.presentationSlotRatio >= MIN_BURST_OUTPUT_LAYER_SLOT_UTILIZATION) {
                "终端内容层只占用了 ${layerStats.presentationSlotRatio} 的刷新槽位，低于下限 " +
                    "${MIN_BURST_OUTPUT_LAYER_SLOT_UTILIZATION}"
            }
            check(windowFrames.isNotEmpty()) {
                "显示级帧时间线在突发输出时间窗内没有帧"
            }
            check(attributedContentFrames.isNotEmpty()) {
                "显示级帧时间线没有把终端内容层归属到被测应用"
            }
            check(samePresentEvents(attributedPresentTimesMillis, layerPresentTimesMillis)) {
                "显示级帧时间线与逐图层通道记录的呈现时刻对不上"
            }
            check(peakPssKilobytes - baselineMemory.pssKilobytes <= MAX_BURST_OUTPUT_PSS_GROWTH_KILOBYTES) {
                "突发输出期间 PSS 增长 " +
                    "${peakPssKilobytes - baselineMemory.pssKilobytes}KB 超过上限 " +
                    "${MAX_BURST_OUTPUT_PSS_GROWTH_KILOBYTES}KB：内存无界"
            }
            check(processCpuMillis <= MAX_BURST_OUTPUT_PROCESS_CPU_MILLIS) {
                "突发输出期间进程 CPU 时间 ${processCpuMillis}ms 超过上限 " +
                    "${MAX_BURST_OUTPUT_PROCESS_CPU_MILLIS}ms"
            }
            mapOf(
                "delivered_bytes" to load.bytes().toDouble(),
                "burst_duration_millis" to burstDurationMillis.toDouble(),
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
                "scrollback_rows" to outcome.scrollbackRows.toDouble(),
            )
        }
    }

    /**
     * 等负载跑完：生产者写完全部字节后打一个完成标记，标记齐了即负载结束。
     *
     * 顺带按固定间隔采内存：峰值只能在这段里取——负载结束后内存可能回落。回滚行数
     * 取应用侧帧计时汇总里的计量，它证明历史有界（回滚区封顶在配置上限）。
     */
    private fun awaitLoadCompletion(device: UiDevice, loadStartMillis: Long): LoadOutcome {
        val deadline = SystemClock.elapsedRealtime() + BURST_OUTPUT_TIMEOUT_MILLIS
        var completed = false
        var completedAtMillis = loadStartMillis
        var peakPssKilobytes = 0L
        var peakRssKilobytes = 0L
        var scrollbackRows = 0
        var nextMemorySampleElapsedMillis = 0L
        while (SystemClock.elapsedRealtime() < deadline) {
            val markers = readLogcatSince(device, loadStartMillis, BURST_OUTPUT_MARKER_PATTERN)
            if (markers.isNotEmpty()) {
                completed = true
                completedAtMillis = markers.last().epochMillis
                break
            }
            val now = SystemClock.elapsedRealtime()
            if (now >= nextMemorySampleElapsedMillis) {
                val memory = readDeviceMemory(device, TARGET_PACKAGE)
                peakPssKilobytes = max(peakPssKilobytes, memory.pssKilobytes.toLong())
                peakRssKilobytes = max(peakRssKilobytes, memory.rssKilobytes.toLong())
                nextMemorySampleElapsedMillis = now + BURST_OUTPUT_MEMORY_SAMPLE_INTERVAL_MILLIS
            }
            SystemClock.sleep(BURST_OUTPUT_POLL_MILLIS)
        }
        readLogcatSince(device, loadStartMillis, SCROLLBACK_ROWS_PATTERN)
            .mapNotNull { record ->
                SCROLLBACK_ROWS_PATTERN.find(record.message)?.groupValues?.get(1)?.toIntOrNull()
            }
            .maxOrNull()
            ?.let { scrollbackRows = it }
        return LoadOutcome(completed, completedAtMillis, peakPssKilobytes, peakRssKilobytes, scrollbackRows)
    }

    /**
     * 把提示符换成不含转义的普通提示符。
     *
     * mkshrc 的提示符每次都带 OSC 0 上报会话标题（当前目录名），流末尾的标题哨兵会被
     * 下一个提示符立刻覆盖，哨兵就留不住。改提示符只影响本会话的显示，不改应用。
     */
    private fun disablePromptTitleReport(device: UiDevice) {
        device.executeShellCommand("input text PS1='>%s'")
        device.executeShellCommand("input keyevent 66")
    }

    /**
     * 等标题哨兵出现在会话抽屉里：抽屉打开时应用强制刷新会话标题，故读到的是当下的
     * 标题而不是打开前的旧值。
     */
    private fun awaitSessionTitleCanary(device: UiDevice, canary: String): Boolean {
        val drawerKey = requireElement(device, DRAWER_KEY_RESOURCE_ID, "抽屉键")
        drawerKey.click()
        requireElement(device, SESSION_DRAWER_RESOURCE_ID, "会话抽屉")
        val deadline = SystemClock.elapsedRealtime() + SESSION_TITLE_TIMEOUT_MILLIS
        var canaryVisible = false
        while (SystemClock.elapsedRealtime() < deadline) {
            if (device.findObject(By.textContains(canary)) != null) {
                canaryVisible = true
                break
            }
            device.waitForIdle()
            SystemClock.sleep(SESSION_TITLE_POLL_MILLIS)
        }
        // 输入法在时返回键先被输入法吃掉（收起输入法），抽屉还在；逐次返回直到抽屉消失。
        var closeAttempts = 0
        while (
            closeAttempts < SESSION_DRAWER_CLOSE_ATTEMPTS &&
            device.findObject(By.res(SESSION_DRAWER_RESOURCE_ID)) != null
        ) {
            device.pressBack()
            device.waitForIdle()
            closeAttempts++
        }
        return canaryVisible
    }

    /** 负载跑完时的取证结果：完成标记、内存峰值与回滚行数。 */
    private data class LoadOutcome(
        /** 生产者是否写完全部字节。 */
        val completed: Boolean,
        /** 完成标记的时刻（epoch 毫秒）。 */
        val completedAtMillis: Long,
        /** 负载进行中采到的 PSS 峰值（千字节）。 */
        val peakPssKilobytes: Long,
        /** 负载进行中采到的 RSS 峰值（千字节）。 */
        val peakRssKilobytes: Long,
        /** 应用侧计量到的回滚行数（取窗口内最大值）。 */
        val scrollbackRows: Int,
    )
}
