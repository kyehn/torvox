package io.term.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.term.cucumber.ComposeRuleHolder
import io.term.openDrawer
import io.term.waitForSession
import javax.inject.Inject

class SessionSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Given("^the app has launched with multiple sessions$")
    fun appHasLaunchedWithMultipleSessions() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("AddSessionButton"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("AddSessionButton", useUnmergedTree = true)
            .performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 12000) {
            val count =
                composeRuleHolder.composeRule
                    .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                    .fetchSemanticsNodes()
                    .size
            count >= 2
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the session drawer is opened$")
    fun sessionDrawerIsOpened() {
        composeRuleHolder.composeRule.openDrawer()
    }

    @When("^the user adds a new session$")
    fun userAddsNewSession() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("AddSessionButton"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("AddSessionButton", useUnmergedTree = true)
            .performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 12000) {
            val count =
                composeRuleHolder.composeRule
                    .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                    .fetchSemanticsNodes()
                    .size
            count >= 2
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user switches to a different session$")
    fun userSwitchesToDifferentSession() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule.waitForIdle()
        val sessionNodes =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        if (sessionNodes.size > 1) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)[1]
                .performClick()
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the session list is displayed$")
    fun sessionListIsDisplayed() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
    }

    @Then("^an \"([^\"]+)\" button exists$")
    fun addSessionButtonExists(buttonText: String) {
        composeRuleHolder.composeRule
            .onNodeWithTag("AddSessionButton")
            .assertIsDisplayed()
    }

    @Then("^both sessions appear in the drawer$")
    fun bothSessionsAppearInDrawer() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        val sessionItems =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        assert(sessionItems.size >= 2) { "Expected at least 2 sessions, found ${sessionItems.size}" }
    }

    @Then("^the terminal shows the new session content$")
    fun terminalShowsNewSessionContent() {
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
}
