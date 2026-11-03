package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val ANIMATION_ITERATION_COUNT = 3
private const val IME_STATE_ATTEMPT_COUNT = 10
private const val EMULATOR_IDLE_TIMEOUT_MILLIS = 2_000L
private const val IME_SHOWN_STATE = "mInputShown=true"

/** `dumpsys gfxinfo` 的帧计数行。 */
private val FRAME_COUNT_PATTERN = Regex("Total frames rendered: (\\d+)")

/** 等修饰键状态生效的上限。 */
private const val SELECTION_TIMEOUT_MILLIS = 3_000L

private const val SELECTION_POLL_INTERVAL_MILLIS = 50L

/**
 * 交互动画的帧级宏基准：
 *
 * 1. 修饰键点击反馈（ModifierBar.kt 中 CTRL 的缩放弹簧 + 背景补间）。
 * 2. IME 弹出动画（Compose 偏移跟随，ModifierBar 随键盘移动）。
 * 3. 会话抽屉展开动画（ModalDrawerSheet）。
 *
 * 每次 measureBlock 内都真实驱动动画，`FrameTimingMetric` 才能采到动画帧。
 * 逐帧数据由指标输出；断言负责证明「动画确实发生了」并拦住卡死级回归：
 * 帧数为 0 说明动画没跑，交互状态没变说明功能坏了。
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
                val controlKey = requireElement(device, CONTROL_KEY_RESOURCE_ID, "CTRL 键")
                resetFrameStats(device)
                val elapsed = elapsedMillis {
                    controlKey.click()
                    device.waitForIdle()
                }
                val frames = readFrameStats(device)
                check(frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "修饰键按下反馈未产生帧：frames=$frames"
                }
                check(elapsed <= INTERACTION_BUDGET_MILLIS) {
                    "修饰键按下反馈耗时 ${elapsed}ms 超过预算 ${INTERACTION_BUDGET_MILLIS}ms"
                }
                check(waitForSelected(device, CONTROL_KEY_RESOURCE_ID, SELECTION_TIMEOUT_MILLIS)) {
                    "点击 CTRL 后应进入粘滞修饰状态"
                }
                // 复位粘滞态，别把 Once 态留给下一次迭代。
                device.findObject(By.res(CONTROL_KEY_RESOURCE_ID))?.click()
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
                requireElement(device, CONTROL_KEY_RESOURCE_ID, "CTRL 键")
                // 启动后输入法自动弹出（DESIGN）：先收起，否则「点终端弹输入法」无事发生，
                // 测到的只是静止画面。
                hideIme(device)
                resetFrameStats(device)
                val elapsed = elapsedMillis {
                    // 点击终端正文请求 IME 焦点，ModifierBar 的偏移动画在测量窗口内运行。
                    device.click(device.displayWidth / 2, device.displayHeight / 3)
                    // 确认输入法真的弹出了，否则测到的动画帧为 0，
                    // 无法区分「跳变动画」与「键盘从未出现」。
                    check(isImeShown(device)) { "点击终端后输入法必须弹出" }
                    device.waitForIdle()
                }
                val frames = readFrameStats(device)
                check(frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "IME 弹出未产生帧：frames=$frames"
                }
                check(elapsed <= INTERACTION_BUDGET_MILLIS) {
                    "IME 弹出耗时 ${elapsed}ms 超过预算 ${INTERACTION_BUDGET_MILLIS}ms"
                }
            },
        )
    }

    @Test
    fun sessionDrawerAnimation() {
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
                val drawerKey = requireElement(device, DRAWER_KEY_RESOURCE_ID, "抽屉键")
                resetFrameStats(device)
                val elapsed = elapsedMillis {
                    drawerKey.click()
                    requireElement(device, SESSION_DRAWER_RESOURCE_ID, "会话抽屉")
                }
                val frames = readFrameStats(device)
                check(frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "会话抽屉展开未产生帧：frames=$frames"
                }
                check(elapsed <= INTERACTION_BUDGET_MILLIS) {
                    "会话抽屉展开耗时 ${elapsed}ms 超过预算 ${INTERACTION_BUDGET_MILLIS}ms"
                }
                device.pressBack()
                device.waitForIdle()
            },
        )
    }

    /** 元素缺失 MUST 失败：找不到元素说明界面没到位，继续测量只会产出无意义的数字。 */
    private fun requireElement(device: UiDevice, resourceId: String, label: String): UiObject2 {
        val element = device.wait(Until.findObject(By.res(resourceId)), UI_TIMEOUT_MILLIS)
        checkNotNull(element) { "未找到$label（$resourceId）" }
        return element
    }

    /**
     * 软件渲染的模拟器连续跑长宏基准会耗尽图形栈，任务以输出插件 EOF 崩溃，
     * 因此每个用例前先强制停止被测应用，再等设备空闲让图形栈恢复。
     */
    private fun recoverEmulator(device: UiDevice) {
        device.executeShellCommand("am force-stop $TARGET_PACKAGE")
        device.waitForIdle(EMULATOR_IDLE_TIMEOUT_MILLIS)
    }

    /**
     * 等粘滞修饰态置起。
     *
     * Compose 的 `selected` 语义映射到无障碍节点的 **checked** 状态（实测：
     * `uiautomator dump` 里 `Key_CTRL` 的 `checkable=true`、`checked` 随粘滞态翻转），
     * 不是 `isSelected`。每次重新查询节点，拿到的是当下的状态。
     */
    private fun waitForSelected(device: UiDevice, resourceId: String, timeoutMillis: Long): Boolean {
        val deadline = SystemClock.elapsedRealtime() + timeoutMillis
        while (SystemClock.elapsedRealtime() < deadline) {
            if (device.findObject(By.res(resourceId))?.isChecked == true) return true
            device.waitForIdle()
            SystemClock.sleep(SELECTION_POLL_INTERVAL_MILLIS)
        }
        return false
    }

    /** 反复查询输入法状态直到弹出；每次 shell 调用本身已提供间隔。 */
    private fun isImeShown(device: UiDevice) = (0 until IME_STATE_ATTEMPT_COUNT).any {
        device.executeShellCommand("dumpsys input_method | grep mInputShown").contains(IME_SHOWN_STATE)
    }

    /**
     * 收起输入法。
     *
     * 输入法可见时 BACK 由输入法消费，不会传给应用；收起失败（输入法仍可见）即视为
     * 测量前置条件不成立，必须失败而不是继续测一段静止画面。
     */
    private fun hideIme(device: UiDevice) {
        device.pressBack()
        device.waitForIdle()
        check(!isImeShown(device)) { "输入法未能收起，无法测量弹出动画" }
    }

    /** 清零被测应用的帧计数，让随后的读数只覆盖本次交互。 */
    private fun resetFrameStats(device: UiDevice) {
        device.executeShellCommand("dumpsys gfxinfo $TARGET_PACKAGE reset")
    }

    /** 读取被测应用自上次清零以来渲染的帧数；读不到时按 0 处理，交给断言判失败。 */
    private fun readFrameStats(device: UiDevice): Int {
        val output = device.executeShellCommand("dumpsys gfxinfo $TARGET_PACKAGE")
        return FRAME_COUNT_PATTERN.find(output)?.groupValues?.get(1)?.toIntOrNull() ?: 0
    }

    private fun elapsedMillis(block: () -> Unit): Long {
        val start = SystemClock.elapsedRealtime()
        block()
        return SystemClock.elapsedRealtime() - start
    }
}
