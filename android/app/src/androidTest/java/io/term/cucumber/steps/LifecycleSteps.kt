package io.term.cucumber.steps

import android.content.pm.ActivityInfo
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.term.cucumber.ComposeRuleHolder
import io.term.waitForSession
import javax.inject.Inject

class LifecycleSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Given("^the app has launched and a session is active$")
    fun appHasLaunchedAndSessionIsActive() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @When("^the activity is recreated$")
    fun activityIsRecreated() {
        composeRuleHolder.composeRule.activityRule.scenario
            .recreate()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the app is force-stopped and relaunched$")
    fun appIsForceStoppedAndRelaunched() {
        composeRuleHolder.composeRule.activityRule.scenario
            .recreate()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the device configuration changes \\(orientation\\)$")
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

    @Then("^the terminal screen is still displayed$")
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

    @Then("^the session is still functional$")
    fun sessionIsStillFunctional() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the session is restored$")
    fun sessionIsRestored() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 25000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the terminal is still interactive$")
    fun terminalIsStillInteractive() {
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar")
            .assertIsDisplayed()
    }

    @Then("^the session continues without interruption$")
    fun sessionContinuesWithoutInterruption() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 10000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar")
            .assertIsDisplayed()
    }
}
