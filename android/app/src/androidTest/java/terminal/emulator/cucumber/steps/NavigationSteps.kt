package terminal.emulator.cucumber.steps

import android.view.KeyEvent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openDrawer
import terminal.emulator.waitForSettingsScreen
import javax.inject.Inject

class NavigationSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @当("^按下返回键$")
    fun backButtonIsPressed() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @当("^打开会话抽屉$")
    fun userOpensSessionDrawer() {
        composeRuleHolder.composeRule.openDrawer()
    }

    @当("^关闭抽屉$")
    fun userClosesDrawer() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @当("^从抽屉进入设置$")
    fun navigatesToSettingsFromDrawer() {
        val rule = composeRuleHolder.composeRule
        // 先直点再兜底，与 TestUtils.openSettings 一致，应对 CI 慢设备。
        // 抽屉内容在关闭时仍被组合，直点常常一步到位。
        val directClickSucceeded =
            runCatching {
                rule.onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
                rule.waitForIdle()
                val probeDeadline = System.currentTimeMillis() + 8000
                while (System.currentTimeMillis() < probeDeadline) {
                    val visible =
                        runCatching {
                            rule
                                .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
                                .assertIsDisplayed()
                            true
                        }
                            .getOrDefault(false)
                    if (visible) return@runCatching true
                    Thread.sleep(200)
                }
                false
            }
                .getOrDefault(false)
        if (directClickSucceeded) {
            rule.waitForSettingsScreen(timeoutMs = 60_000)
            return
        }
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val uiDirect =
            runCatching {
                if (device.wait(Until.hasObject(By.text("设置")), 5000)) {
                    device.findObject(By.text("设置"))?.click()
                    rule.waitForIdle()
                    Thread.sleep(500)
                    runCatching {
                        rule
                            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
                            .assertIsDisplayed()
                    }
                        .isSuccess || device.hasObject(By.text("字体"))
                } else {
                    false
                }
            }
                .getOrDefault(false)
        if (uiDirect) {
            rule.waitForSettingsScreen(timeoutMs = 60_000)
            return
        }
        var settingsReached = false
        repeat(3) {
            rule.openDrawer()
            if (device.wait(Until.hasObject(By.text("设置")), 10_000)) {
                device.findObject(By.text("设置"))?.click()
            } else if (device.wait(Until.hasObject(By.desc("设置")), 10_000)) {
                device.findObject(By.desc("设置"))?.click()
            } else if (device.wait(Until.hasObject(By.res("SettingsButton")), 10_000)) {
                device.findObject(By.res("SettingsButton"))?.click()
            }
            rule.waitForIdle()
            Thread.sleep(500)
            settingsReached =
                runCatching {
                    rule.waitForSettingsScreen(timeoutMs = 15_000)
                    true
                }
                    .getOrDefault(false) ||
                device.hasObject(By.text("字体")) ||
                device.hasObject(By.res("SettingsScreen"))
            if (settingsReached) return@repeat
            Thread.sleep(1000)
        }
        if (!settingsReached) {
            rule.waitForSettingsScreen(timeoutMs = 60_000)
        }
    }

    @那么("^抽屉已显示$")
    fun drawerIsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SessionDrawer", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @那么("^终端界面完全可见$")
    fun terminalScreenIsFullyVisible() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @那么("^设置界面已显示$")
    fun settingsScreenIsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
