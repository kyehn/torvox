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

private const val ANIMATION_ITERATION_COUNT = 3
private const val UI_TIMEOUT_MILLIS = 10_000L
private const val IME_STATE_ATTEMPT_COUNT = 10
private const val EMULATOR_IDLE_TIMEOUT_MILLIS = 2_000L
private const val IME_SHOWN_STATE = "mInputShown=true"

/**
 * 交互动画的帧级宏基准：
 *
 * 1. 修饰键点击反馈（ModifierBar.kt 中 CTRL 的缩放弹簧 + 背景补间）。
 * 2. IME 弹出动画（Compose 偏移跟随，ModifierBar 随键盘移动）。
 *
 * 每次 measureBlock 内都真实驱动动画，`FrameTimingMetric` 才能采到动画帧。
 * 本模块只输出指标、不设阈值：CI 用的是 swiftshader 软件渲染模拟器，
 * 其帧率不能作为回归判据，阈值需在真机上另行标定。
 */
@RunWith(AndroidJUnit4::class)
class InteractionAnimationBenchmark {
    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun modifierKeyPressAnimation() {
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(FrameTimingMetric()),
            iterations = ANIMATION_ITERATION_COUNT,
            setupBlock = {
                grantNotificationPermission(device)
                recoverEmulator(device)
            },
            measureBlock = {
                startActivityAndWait()
                val controlKey = waitForControlKey(device)
                controlKey?.click()
                device.waitForIdle()
                device.pressBack()
                device.waitForIdle()
            },
        )
    }

    @Test
    fun imeShowAnimation() {
        benchmarkRule.measureRepeated(
            packageName = TARGET_PACKAGE,
            metrics = listOf(FrameTimingMetric()),
            iterations = ANIMATION_ITERATION_COUNT,
            setupBlock = {
                grantNotificationPermission(device)
                recoverEmulator(device)
            },
            measureBlock = {
                startActivityAndWait()
                waitForControlKey(device)
                // 点击终端正文请求 IME 焦点，ModifierBar 的偏移动画在测量窗口内运行。
                device.click(device.displayWidth / 2, device.displayHeight / 3)
                // 确认输入法真的弹出了，否则测到的动画帧为 0，
                // 无法区分「跳变动画」与「键盘从未出现」。
                check(isImeShown(device)) { "点击终端后输入法必须弹出" }
                device.waitForIdle()
            },
        )
    }

    /** ModifierBar 由 testTagsAsResourceId 暴露 resource-id，避免硬编码坐标。 */
    private fun waitForControlKey(device: UiDevice) =
        device.wait(Until.findObject(By.res(CONTROL_KEY_RESOURCE_ID)), UI_TIMEOUT_MILLIS)

    /**
     * 软件渲染的模拟器连续跑长宏基准会耗尽图形栈，任务以输出插件 EOF 崩溃，
     * 因此每个用例前先强制停止被测应用，再等设备空闲让图形栈恢复。
     */
    private fun recoverEmulator(device: UiDevice) {
        device.executeShellCommand("am force-stop $TARGET_PACKAGE")
        device.waitForIdle(EMULATOR_IDLE_TIMEOUT_MILLIS)
    }

    /** 反复查询输入法状态直到弹出；每次 shell 调用本身已提供间隔。 */
    private fun isImeShown(device: UiDevice) = (0 until IME_STATE_ATTEMPT_COUNT).any {
        device.executeShellCommand("dumpsys input_method | grep mInputShown").contains(IME_SHOWN_STATE)
    }
}
