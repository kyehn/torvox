package io.torvox.cucumber.steps

import android.view.KeyEvent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.isDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.torvox.cucumber.ComposeRuleHolder
import io.torvox.openDrawer
import io.torvox.openSettings
import io.torvox.waitForSession
import javax.inject.Inject

class NavigationSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the back button is pressed$")
    fun backButtonIsPressed() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user opens the session drawer$")
    fun userOpensSessionDrawer() {
        composeRuleHolder.composeRule.openDrawer()
    }

    @When("^the user closes the drawer$")
    fun userClosesDrawer() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^navigates to settings from the drawer$")
    fun navigatesToSettingsFromDrawer() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsButton")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the drawer is displayed$")
    fun drawerIsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SessionDrawer", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the terminal screen is fully visible$")
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

    @Then("^the settings screen is displayed$")
    fun settingsScreenIsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
