package terminal.emulator.benchmark

import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BridgeMicrobenchmark {
    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    private fun grantNotificationPermission(
        device: androidx.test.uiautomator.UiDevice,
    ) {
        // Macrobenchmark (re)installs the target app, which resets
        // runtime permissions; MainActivity.onCreate() then shows the
        // POST_NOTIFICATIONS dialog and startup metrics read zero
        // (round-119). Grant before measuring — idempotent.
        device.executeShellCommand("pm grant com.termux android.permission.POST_NOTIFICATIONS")
    }

    @Test
    fun coldStart() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(StartupTimingMetric(), FrameTimingMetric()),
            iterations = 10,
            setupBlock = {
                grantNotificationPermission(device)
                device.pressHome()
            },
            measureBlock = {
                startActivityAndWait()
            },
        )
    }

    @Test
    fun warmStart() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(StartupTimingMetric()),
            iterations = 10,
            setupBlock = {
                grantNotificationPermission(device)
                startActivityAndWait()
                device.waitForIdle()
                device.pressHome()
            },
            measureBlock = {
                startActivityAndWait()
            },
        )
    }

    @Test
    fun terminalOutputTiming() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(StartupTimingMetric()),
            iterations = 10,
            setupBlock = {
                grantNotificationPermission(device)
                device.pressHome()
            },
            measureBlock = {
                startActivityAndWait()
            },
        )
    }
}
