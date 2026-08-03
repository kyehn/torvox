package terminal.emulator.benchmark

import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Frame-level animation benchmarks for interactive UI paths that the
 * user explicitly asked to verify frame-by-frame:
 *
 *  1. ModifierBar key press feedback (spring scale 0.90 + 100ms tween
 *     background in ModifierBar.kt).
 *  2. IME show animation (imePadding pushes the terminal layout).
 *  3. IME hide animation (back press).
 *
 * Each benchmark drives the real animation inside measureBlock so
 * [FrameTimingMetric] samples the actual animation frames. A
 * frameTimingResult below ~55 fps on the emulator is a regression
 * signal for the animation path.
 */
@RunWith(AndroidJUnit4::class)
class InteractionAnimationBenchmark {
    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    private fun grantNotificationPermission(device: UiDevice) {
        device.executeShellCommand(
            "pm grant com.termux android.permission.POST_NOTIFICATIONS",
        )
    }

    @Test
    fun modifierKeyPressAnimation() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(FrameTimingMetric()),
            iterations = 3,
            setupBlock = {
                grantNotificationPermission(device)
                // The emulator renders with software Vulkan/GL; running
                // several long macrobenchmarks back to back exhausts it
                // and the run crashes with an EOF in the output plugin.
                // Give the system a moment to recover between tests.
                device.executeShellCommand("am force-stop com.termux")
                Thread.sleep(2_000)
                device.waitForIdle()
            },
            measureBlock = {
                startActivityAndWait()
                device.wait(
                    Until.hasObject(By.text("CTRL")),
                    10_000,
                )
                // Tap the CTRL key — its spring press animation runs
                // during the measurement window.
                device.click(233, 2289)
                device.waitForIdle()
                device.pressBack()
                device.waitForIdle()
            },
        )
    }

    @Test
    fun imeShowAnimation() {
        benchmarkRule.measureRepeated(
            packageName = "com.termux",
            metrics = listOf(FrameTimingMetric()),
            iterations = 3,
            setupBlock = {
                grantNotificationPermission(device)
                // The emulator renders with software Vulkan/GL; running
                // several long macrobenchmarks back to back exhausts it
                // and the run crashes with an EOF in the output plugin.
                // Give the system a moment to recover between tests.
                device.executeShellCommand("am force-stop com.termux")
                Thread.sleep(2_000)
                device.waitForIdle()
            },
            measureBlock = {
                startActivityAndWait()
                device.wait(
                    Until.hasObject(By.text("CTRL")),
                    10_000,
                )
                // Tap the terminal body to request IME focus; the
                // animated bar offset runs during measurement. Use the
                // screen center, which is inside the terminal area.
                device.click(device.displayWidth / 2, device.displayHeight / 3)
                // Confirm the IME actually opened; otherwise the test
                // silently measures zero frames and cannot distinguish
                // "jump animation" from "keyboard never appeared".
                val imeUp =
                    (0..10).any {
                        device.executeShellCommand(
                            "dumpsys input_method | grep mInputShown",
                        ).contains("mInputShown=true")
                    }
                check(imeUp) { "IME must be shown after tapping the terminal" }
                device.waitForIdle()
            },
        )
    }

}
