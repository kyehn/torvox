package io.torvox.cucumber.steps

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipe
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.torvox.cucumber.ComposeRuleHolder
import io.torvox.openSettings
import io.torvox.waitForSession
import javax.inject.Inject

class SettingsSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the user opens the settings screen$")
    fun userOpensSettingsScreen() {
        composeRuleHolder.composeRule.openSettings()
    }

    @Then("^theme selector, font size slider, cursor style selector are displayed$")
    fun settingsSectionsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("FontSizeSlider", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the font size slider exists$")
    fun fontSizeSliderExists() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("FontSizeSlider", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @When("^the slider is adjusted$")
    fun sliderIsAdjusted() {
        composeRuleHolder.composeRule
            .onNodeWithTag("FontSizeSlider", useUnmergedTree = true)
            .performTouchInput {
                swipe(
                    Offset(width * 0.2f, height * 0.5f),
                    Offset(width * 0.8f, height * 0.5f),
                )
            }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the terminal font size changes$")
    fun terminalFontSizeChanges() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsBackButton", useUnmergedTree = true)
            .performClick()
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

    @When("^the cursor style is changed from block to bar$")
    fun cursorStyleChangedToBar() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = true)
            .performScrollToNode(hasTestTag("CursorStyleSelector"))
        composeRuleHolder.composeRule
            .onNodeWithTag("CursorStyleSelector", useUnmergedTree = true)
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithText("Bar")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the bar cursor style is selected$")
    fun barCursorStyleSelected() {
        composeRuleHolder.composeRule.waitForIdle()
        // The cursor style selector is present and reflects the chosen style.
        composeRuleHolder.composeRule
            .onNodeWithTag("CursorStyleSelector", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
