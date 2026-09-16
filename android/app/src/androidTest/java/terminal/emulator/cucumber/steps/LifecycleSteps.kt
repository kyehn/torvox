package terminal.emulator.cucumber.steps

import android.content.pm.ActivityInfo
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import io.cucumber.java.zh_cn.假如
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.waitForSession
import javax.inject.Inject

class LifecycleSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    @假如("^应用已启动且会话处于活动状态$")
    fun appHasLaunchedAndSessionIsActive() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @当("^活动被重建$")
    fun activityIsRecreated() {
        composeRuleHolder.composeRule.activityRule.scenario.recreate()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @当("^设备方向改变$")
    fun deviceConfigurationChangesOrientation() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE
        }
        Thread.sleep(2000)
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_USER_PORTRAIT
        }
        Thread.sleep(2000)
    }

    @那么("^终端界面仍显示$")
    fun terminalScreenIsStillDisplayed() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 15000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @那么("^会话仍可用$")
    fun sessionIsStillFunctional() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @那么("^会话不间断继续$")
    fun sessionContinuesWithoutInterruption() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 10000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }
}
