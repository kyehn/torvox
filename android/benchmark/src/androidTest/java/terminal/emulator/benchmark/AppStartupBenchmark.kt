package terminal.emulator.benchmark

import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * App-level macrobenchmarks: cold/warm startup and terminal output timing.
 *
 * Named AppStartupBenchmark (not Bridge*) because these measure app
 * lifecycle phases, not JNI bridge call-level costs. Bridge call-level
 * measurements live in the Rust bench suite (render/terminal benches) and
 * the JNI integration tests; a device-side microbenchmark of raw JNI
 * round-trips would need a benchmark APK sharing the app process and is
 * tracked as a follow-up (docs/test-coverage-audit.md P1-7).
 */
@RunWith(AndroidJUnit4::class)
class AppStartupBenchmark {
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
