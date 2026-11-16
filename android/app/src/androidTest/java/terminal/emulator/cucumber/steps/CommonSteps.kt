package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openSettings
import terminal.emulator.waitForSession
import javax.inject.Inject

class CommonSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    @Given("^应用已启动$")
    fun appHasLaunched() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @Given("^用户在设置界面$")
    fun userIsOnSettingsScreen() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.openSettings()
    }

    @Then("^终端界面已显示$")
    fun terminalScreenIsDisplayed() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertExists()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
