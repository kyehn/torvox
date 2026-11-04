package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val ANIMATION_ITERATION_COUNT = 3

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
                val frames = readFrameStatsAfterRender(device)
                // 实测值先于断言输出：断言失败时构建产物里仍留得下这轮的数字。
                printMeasured("modifierKeyPress", elapsed, frames)
                check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "修饰键按下反馈未产生帧：frames=${frames.frames}"
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
                val frames = readFrameStatsAfterRender(device)
                printMeasured("imeShow", elapsed, frames)
                check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "IME 弹出未产生帧：frames=${frames.frames}"
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
                val frames = readFrameStatsAfterRender(device)
                printMeasured("sessionDrawer", elapsed, frames)
                check(frames.frames >= MIN_ANIMATION_FRAME_COUNT) {
                    "会话抽屉展开未产生帧：frames=${frames.frames}"
                }
                check(elapsed <= INTERACTION_BUDGET_MILLIS) {
                    "会话抽屉展开耗时 ${elapsed}ms 超过预算 ${INTERACTION_BUDGET_MILLIS}ms"
                }
                device.pressBack()
                device.waitForIdle()
            },
        )
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

    /**
     * 收起输入法。
     *
     * 输入法可见时 BACK 由输入法消费，不会传给应用；收起失败（输入法仍可见）即视为
     * 测量前置条件不成立，必须失败而不是继续测一段静止画面。
     */
    private fun hideIme(device: UiDevice) {
        device.pressBack()
        device.waitForIdle()
        check(isImeHidden(device)) { "输入法未能收起，无法测量弹出动画" }
    }
}
