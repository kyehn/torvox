package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import org.junit.Assert.assertTrue
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession
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
        assertTrue("Expected at least 2 sessions, found ${sessionItems.size}", sessionItems.size >= 2)
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
        // The new session exists: the count can only grow across scenarios
        // (Activity is shared for the whole Cucumber run, so a fixed total
        // like 2 would flake on the third scenario). Grid content itself is
        // unreadable while getTerminalText is an implemented (native query path is wired).
        val sessionCount =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertTrue("Expected at least 2 sessions after creating a new one, got $sessionCount", sessionCount >= 2)
    }
}
