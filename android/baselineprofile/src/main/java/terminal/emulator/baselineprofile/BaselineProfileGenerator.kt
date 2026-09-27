package terminal.emulator.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private const val TARGET_PACKAGE = "com.termux"
private const val NOTIFICATION_PERMISSION = "android.permission.POST_NOTIFICATIONS"
private const val PROFILE_ITERATION_COUNT = 15
private const val UI_TIMEOUT_MILLIS = 10_000L
private const val SETTLE_MILLIS = 1_000L

/** Compose `testTag` 经 `testTagsAsResourceId` 暴露为 resource-id，可直接用于 UiAutomator 定位。 */
private const val MODIFIER_BAR_RESOURCE_ID = "ModifierBar"
private const val CONTROL_KEY_RESOURCE_ID = "Key_CTRL"
private const val DRAWER_KEY_RESOURCE_ID = "Key_DRAWER"
private const val SESSION_DRAWER_RESOURCE_ID = "SessionDrawer"

/** `input text` 以 %s 表示空格；命令在 shell 中执行，可走通 PTY 写入、VT 解析与渲染。 */
private const val PROFILE_COMMAND_TEXT = "echo%sbaseline_profile"

private const val ENTER_KEY_EVENT = "KEYCODE_ENTER"

/**
 * Baseline profile 采集：用 UiAutomator 走一段真实路径，把启动、终端渲染、修饰键、
 * 会话抽屉上的类与方法标记为热，供 ART 提前 AOT 编译。
 *
 * 真机执行 `./gradlew generateBaselineProfile`；产物由 app 侧
 * `baselineProfile { saveInSrc = true }` 写入 `app/src/main/baselineProfiles/`，
 * 与手写规则同目录，由 AGP 合并进 `assets/dexopt/baseline.prof`。
 *
 * 全程不断言：未安装引导程序、首次启动弹窗等环境差异不应让整次采集失败。
 */
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val baselineProfileRule = BaselineProfileRule()

    @Test
    fun generate() {
        baselineProfileRule.collect(
            packageName = TARGET_PACKAGE,
            maxIterations = PROFILE_ITERATION_COUNT,
        ) {
            // 重装被测应用会重置运行时权限；不授权则首启弹通知对话框，启动路径与真机不一致。
            device.executeShellCommand("pm grant $TARGET_PACKAGE $NOTIFICATION_PERMISSION")
            startActivityAndWait()
            // 修饰栏是首帧后最重的 Compose 子树，用作「已启动」锚点。
            device.wait(Until.hasObject(By.res(MODIFIER_BAR_RESOURCE_ID)), UI_TIMEOUT_MILLIS)
            focusTerminal(device)
            runProfileCommand(device)
            clickByResourceId(device, CONTROL_KEY_RESOURCE_ID)
            openSessionDrawer(device)
            pressHome()
        }
    }

    /** 点击终端正文唤起 IME，输入法弹出动画与取焦路径同样需要预热。 */
    private fun focusTerminal(device: UiDevice) {
        device.click(device.displayWidth / 2, device.displayHeight / 3)
        device.waitForIdle(SETTLE_MILLIS)
    }

    /** 经 IME 提交文本，真实走通 IME → PTY 写入 → VT 解析 → 渲染整条链路。 */
    private fun runProfileCommand(device: UiDevice) {
        device.executeShellCommand("input text $PROFILE_COMMAND_TEXT")
        device.executeShellCommand("input keyevent $ENTER_KEY_EVENT")
        device.waitForIdle(SETTLE_MILLIS)
    }

    private fun openSessionDrawer(device: UiDevice) {
        clickByResourceId(device, DRAWER_KEY_RESOURCE_ID)
        device.wait(Until.hasObject(By.res(SESSION_DRAWER_RESOURCE_ID)), UI_TIMEOUT_MILLIS)
        device.pressBack()
        device.waitForIdle()
    }

    private fun clickByResourceId(device: UiDevice, resourceId: String) {
        device.findObject(By.res(resourceId))?.click()
        device.waitForIdle(SETTLE_MILLIS)
    }
}
