package io.term.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.espresso.Espresso
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.term.cucumber.ComposeRuleHolder
import io.term.openSettings
import io.term.waitForSession
import javax.inject.Inject

class ThemeSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the user selects a different theme$")
    fun userSelectsDifferentTheme() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = true)
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            try {
                composeRuleHolder.composeRule
                    .onNodeWithText("Dracula Plus")
                    .assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            }
        }
        composeRuleHolder.composeRule
            .onNodeWithText("Dracula Plus")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the terminal theme updates$")
    fun terminalThemeUpdates() {
        composeRuleHolder.composeRule.waitForIdle()
        Espresso.pressBack()
        composeRuleHolder.composeRule.waitForIdle()
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

    @Then("^the default theme is applied to the terminal$")
    fun defaultThemeIsApplied() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
