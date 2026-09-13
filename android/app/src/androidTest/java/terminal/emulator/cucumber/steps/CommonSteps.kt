package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import io.cucumber.java.zh_cn.假如
import io.cucumber.java.zh_cn.那么
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openSettings
import terminal.emulator.waitForSession
import javax.inject.Inject

class CommonSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @假如("^应用已启动$")
    fun appHasLaunched() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @假如("^用户在设置界面$")
    fun userIsOnSettingsScreen() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.openSettings()
    }

    @那么("^终端界面已显示$")
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
