package io.term.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.term.cucumber.ComposeRuleHolder
import io.term.openSettings
import io.term.waitForSession
import javax.inject.Inject

class CommonSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Given("^the app has launched$")
    fun appHasLaunched() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @Given("^the user is on the settings screen$")
    fun userIsOnSettingsScreen() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.openSettings()
    }

    @Then("^the terminal screen is displayed$")
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
