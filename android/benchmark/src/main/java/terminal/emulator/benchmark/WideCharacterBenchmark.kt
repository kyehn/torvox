package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class WideCharacterBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun wideCharacterLines() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("wideCharacter")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        var sessionStartMillis = 0L
        lateinit var contentLayer: String
        measureScenario(
            device = device,
            label = "wideCharacter",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S8"),
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                grantNotificationPermission(device)
                recoverEmulator(device)
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                contentLayer = awaitTerminalContentFrames(device).first
                awaitShellReady(device, sessionStartMillis)
                disablePromptTitleReport(device)
            },
            expectProcessExit = false,
        ) { _, budget ->
            val load = WideCharacterLoad(lineCount = WIDE_CHARACTER_LINE_COUNT, canary = WIDE_CHARACTER_CANARY)
            check(load.bytes() <= scenarioById("S8").outputByteLimit) {
                "宽字符负载 ${load.bytes()} 字节超过场景上限 ${scenarioById("S8").outputByteLimit}"
            }
            device.executeShellCommand("input text ${load.inputTextArgument()}")
            clearLayerFrameHistory(device, contentLayer)
            resetFrameStats(device)
            val cpuBefore = readProcessCpuTime(device, TARGET_PACKAGE)
            val baselineMemory = readDeviceMemory(device, TARGET_PACKAGE)
            val loadStartMillis = System.currentTimeMillis()
            val loadStartElapsedMillis = SystemClock.elapsedRealtime()
            device.executeShellCommand("input keyevent 66")
            val outcome = awaitOutputLoadCompletion(
                device = device,
                packageName = TARGET_PACKAGE,
                loadStartMillis = loadStartMillis,
                markerPattern = Regex(WIDE_CHARACTER_MARKER),
                expectedMarkerCount = 1,
                timeoutMillis = WIDE_CHARACTER_TIMEOUT_MILLIS,
                pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
                memorySampleIntervalMillis = SUSTAINED_OUTPUT_MEMORY_SAMPLE_INTERVAL_MILLIS,
                budget = budget,
            )
            val timelineReadElapsedMillis = SystemClock.elapsedRealtime()
            val summaries = awaitPresentedSettle(device, loadStartMillis)
            val frames = collectOutputScenarioFrames(
                device = device,
                packageName = TARGET_PACKAGE,
                contentLayer = contentLayer,
                loadStartMillis = loadStartMillis,
                loadStartElapsedMillis = loadStartElapsedMillis,
                timelineReadElapsedMillis = timelineReadElapsedMillis,
                summaries = summaries,
            )
            val cpuAfter = readProcessCpuTime(device, TARGET_PACKAGE)
            val processCpuMillis = cpuAfter.totalMillis - cpuBefore.totalMillis
            val peakPssKilobytes = maxOf(outcome.peakPssKilobytes, baselineMemory.pssKilobytes.toLong())
            val canaryVisible = awaitSessionTitleCanary(device, load.canary)
            val artifacts = mutableListOf<ArtifactRef>()
            captureScreenshotArtifact(device, store, "wideCharacter.png", artifacts)
            printMeasured("wideCharacter", outcome.completedAtMillis - loadStartMillis, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("wideCharacter", outcome.completedAtMillis - loadStartMillis) +
                    " canaryVisible=$canaryVisible" +
                    " processCpu=${processCpuMillis}ms" +
                    " peakPss=${peakPssKilobytes}KB",
            )
            check(outcome.markers.size >= 1) {
                "宽字符负载未完成：没有收到完成标记"
            }
            check(canaryVisible) {
                "宽字符标题哨兵没有留在会话标题里：排版或标题上报出了问题"
            }
            assertOutputScenarioFrames(
                frames = frames,
                minPresentedWindows = WIDE_CHARACTER_MIN_PRESENTED_WINDOWS,
                maxPresentationGapMillis = WIDE_CHARACTER_MAX_PRESENTATION_GAP_MILLIS,
                maxLayerIntervalP95Millis = WIDE_CHARACTER_MAX_LAYER_INTERVAL_P95_MILLIS,
                minLayerSlotRatio = WIDE_CHARACTER_MIN_LAYER_SLOT_RATIO,
            )
            check(peakPssKilobytes - baselineMemory.pssKilobytes <= WIDE_CHARACTER_MAX_PSS_GROWTH_KILOBYTES) {
                "宽字符期间 PSS 增长 ${peakPssKilobytes - baselineMemory.pssKilobytes}KB 超过上限 $WIDE_CHARACTER_MAX_PSS_GROWTH_KILOBYTES KB"
            }
            check(processCpuMillis <= WIDE_CHARACTER_MAX_PROCESS_CPU_MILLIS) {
                "宽字符期间进程 CPU 时间 ${processCpuMillis}ms 超过上限 $WIDE_CHARACTER_MAX_PROCESS_CPU_MILLIS ms"
            }
            mapOf(
                "delivered_bytes" to load.bytes().toDouble(),
                "presented_frames" to frames.presentedFrames.toDouble(),
                "presented_windows" to frames.presentedWindows.toDouble(),
                "layer_interval_p95_millis" to frames.layerStats.p95IntervalMillis.toDouble(),
                "layer_slot_ratio" to frames.layerStats.presentationSlotRatio,
                "process_cpu_millis" to processCpuMillis.toDouble(),
                "peak_pss_kilobytes" to peakPssKilobytes.toDouble(),
            )
        }
    }
}
