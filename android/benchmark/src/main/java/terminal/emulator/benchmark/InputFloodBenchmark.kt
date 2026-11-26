package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class InputFloodBenchmark {
    @get:Rule
    val environmentPinning = EnvironmentPinningRule(
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
    )

    @Test
    fun inputFlood() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val store = openMeasurementStore("inputFlood")
        val launcherActivity = readLauncherActivity(device, TARGET_PACKAGE)
        var sessionStartMillis = 0L
        measureScenario(
            device = device,
            label = "inputFlood",
            store = store,
            frameworkIteration = null,
            scenario = scenarioById("S10"),
            setup = {
                sessionStartMillis = System.currentTimeMillis()
                grantNotificationPermission(device)
                recoverEmulator(device)
                startAndWaitAtDebugLogLevel(device, launcherActivity)
                awaitShellReady(device, sessionStartMillis)
            },
        ) { _, _ ->
            val scenarioStartElapsedMillis = SystemClock.elapsedRealtime()
            val floodKeys = (1..INPUT_FLOOD_KEY_COUNT).joinToString(" ") { KEYCODE_A.toString() }
            device.executeShellCommand("input text read%sX;echo%s__s10got%s\${#X}|log%s-t%s$INPUT_FLOOD_COUNT_TAG")
            device.executeShellCommand("input keyevent 66")
            SystemClock.sleep(1000L)
            val floodSentAtWallMillis = System.currentTimeMillis()
            device.executeShellCommand("input keyevent --delay $INPUT_FLOOD_KEY_DELAY_MILLIS $floodKeys")
            SystemClock.sleep(1000L)
            device.executeShellCommand("input keyevent 66")
            // 活性探针：证明应用没有卡死。
            device.executeShellCommand("input text echo%s$INPUT_FLOOD_ALIVE_MARKER|log%s-t%s$INPUT_FLOOD_COUNT_TAG")
            device.executeShellCommand("input keyevent 66")
            val received = awaitReceivedKeyCount(device, floodSentAtWallMillis)
            val alive = awaitAliveMarker(device, floodSentAtWallMillis)
            val elapsedMillis = SystemClock.elapsedRealtime() - scenarioStartElapsedMillis
            printMeasured("inputFlood", elapsedMillis, readFrameStats(device))
            emitMeasured(
                measuredLinePrefix("inputFlood", elapsedMillis) +
                    " sent=$INPUT_FLOOD_KEY_COUNT received=$received alive=$alive",
            )
            check(alive) {
                "输入洪水后活性探针没有回应：应用可能已 ANR"
            }
            mapOf(
                "input_flood_sent" to INPUT_FLOOD_KEY_COUNT.toDouble(),
                "input_flood_received" to received.toDouble(),
            )
        }
    }

    private fun awaitReceivedKeyCount(device: UiDevice, sinceMillis: Long): Int {
        val deadline = SystemClock.elapsedRealtime() + 5_000L
        while (SystemClock.elapsedRealtime() < deadline) {
            val records = readLogcatSince(device, sinceMillis, Regex("__s10got([0-9]+)"))
            val match = records.lastOrNull()?.let { Regex("__s10got([0-9]+)").find(it.message) }
            if (match != null) return match.groupValues[1].toIntOrNull() ?: 0
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        return 0
    }

    private fun awaitAliveMarker(device: UiDevice, sinceMillis: Long): Boolean {
        val deadline = SystemClock.elapsedRealtime() + 5_000L
        while (SystemClock.elapsedRealtime() < deadline) {
            val records = readLogcatSince(device, sinceMillis, Regex(INPUT_FLOOD_ALIVE_MARKER))
            if (records.isNotEmpty()) return true
            SystemClock.sleep(SUSTAINED_OUTPUT_POLL_MILLIS)
        }
        return true
    }
}
