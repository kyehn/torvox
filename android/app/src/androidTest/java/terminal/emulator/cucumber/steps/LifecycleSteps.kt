package terminal.emulator.cucumber.steps

import android.content.pm.ActivityInfo
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.assertTerminalScreenDisplayed
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalScreenNode
import javax.inject.Inject

class LifecycleSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    @Given("^应用已启动且会话处于活动状态$")
    fun appHasLaunchedAndSessionIsActive() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @When("^活动被重建$")
    fun activityIsRecreated() {
        composeRuleHolder.composeRule.activityRule.scenario
            .recreate()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^设备方向改变$")
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

    @Then("^终端界面仍显示$")
    fun terminalScreenIsStillDisplayed() {
        composeRuleHolder.composeRule.waitForTerminalScreenNode(15_000)
        composeRuleHolder.composeRule.assertTerminalScreenDisplayed()
    }

    @Then("^会话仍可用$")
    fun sessionIsStillFunctional() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.assertTerminalScreenDisplayed()
    }

    @Then("^会话不间断继续$")
    fun sessionContinuesWithoutInterruption() {
        composeRuleHolder.composeRule.waitForTerminalScreenNode(10_000)
        composeRuleHolder.composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }
}
