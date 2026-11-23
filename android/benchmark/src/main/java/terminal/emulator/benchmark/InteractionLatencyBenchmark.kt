package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class InteractionLatencyBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun interactionLatency() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("interactionLatency")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        var sessionStartMillis = 0L
        lateinit var contentLayer: String
        measureScenario(
            device = device,
            label = "interactionLatency",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S9"),
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                grantNotificationPermission(device)
                recoverEmulator(device)
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                contentLayer = awaitTerminalContentFrames(device).first
                awaitShellReady(device, sessionStartMillis)
            },
        ) { iteration, _ ->
            val echoLatencies = mutableListOf<Long>()
            val presentLatencies = mutableListOf<Long>()
            repeat(INTERACTION_LATENCY_ROUND_COUNT) { round ->
                clearLayerFrameHistory(device, contentLayer)
                val marker = "__s9probe${iteration}_$round"
                val presentStartElapsedMillis = SystemClock.elapsedRealtime()
                val sentAtWallMillis = System.currentTimeMillis()
                device.executeShellCommand("input text log%s-t%s$INTERACTION_LATENCY_PROBE_TAG%s$marker")
                device.executeShellCommand("input keyevent 66")
                // 回显延迟：logcat 墙钟。
                val echoRecord =
                    awaitEchoMarker(device, sentAtWallMillis, marker, INTERACTION_LATENCY_ECHO_TIMEOUT_MILLIS)
                val echoLatency =
                    echoRecord?.epochMillis?.minus(sentAtWallMillis) ?: INTERACTION_LATENCY_ECHO_TIMEOUT_MILLIS
                echoLatencies += echoLatency
                // 呈现延迟：按键到首个呈现（逐图层通道单调时钟）。
                val firstPresentNanos = awaitLayerPresentAfter(
                    device = device,
                    layerName = contentLayer,
                    sinceMillis = presentStartElapsedMillis,
                    timeoutMillis = INTERACTION_LATENCY_PRESENT_TIMEOUT_MILLIS,
                    pollMillis = SUSTAINED_OUTPUT_POLL_MILLIS,
                )
                val presentLatency = firstPresentNanos / 1_000_000L - presentStartElapsedMillis
                presentLatencies += presentLatency
                emitMeasured(
                    "[bench-measure] interactionLatency round=$round echo=${echoLatency}ms present=${presentLatency}ms",
                )
            }
            val echoP95 = percentile(echoLatencies, 0.95)
            val presentP95 = percentile(presentLatencies, 0.95)
            printMeasured("interactionLatency", echoP95, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("interactionLatency", echoP95) +
                    " echoP95=${echoP95}ms presentP95=${presentP95}ms",
            )
            check(echoP95 <= MAX_INTERACTION_ECHO_LATENCY_MILLIS) {
                "交互回显延迟 p95 ${echoP95}ms 超过上限 $MAX_INTERACTION_ECHO_LATENCY_MILLIS ms"
            }
            check(presentP95 <= MAX_INTERACTION_PRESENT_LATENCY_MILLIS) {
                "按键到首个呈现 p95 ${presentP95}ms 超过上限 $MAX_INTERACTION_PRESENT_LATENCY_MILLIS ms"
            }
            mapOf(
                "interaction_echo_latency_p95_millis" to echoP95.toDouble(),
                "interaction_present_latency_p95_millis" to presentP95.toDouble(),
            )
        }
    }

    private fun awaitEchoMarker(
        device: UiDevice,
        sinceMillis: Long,
        marker: String,
        timeoutMillis: Long,
    ): LogcatRecord? {
        val deadline = SystemClock.elapsedRealtime() + timeoutMillis
        while (SystemClock.elapsedRealtime() < deadline) {
            val records = readLogcatSince(device, sinceMillis, Regex(marker))
            if (records.isNotEmpty()) return records.last()
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        return null
    }
}
