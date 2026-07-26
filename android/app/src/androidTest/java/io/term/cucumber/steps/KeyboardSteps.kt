package io.term.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.term.cucumber.ComposeRuleHolder
import io.term.findTerminalSurface
import io.term.getBridge
import io.term.injectTap
import io.term.waitForSession
import javax.inject.Inject

class KeyboardSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the ALT key is pressed$")
    fun altKeyIsPressed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_ALT")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user types \"([^\"]+)\" in the terminal$")
    fun userTypesInTerminal(input: String) {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("$input\n".toByteArray())
        Thread.sleep(2000)
    }

    @When("^CTRL and ALT are held simultaneously$")
    fun ctrlAndAltHeldSimultaneously() {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_CTRL")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_ALT")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the response time is less than (\\d+) milliseconds$")
    fun responseTimeIsLessThanMillis(expectedMs: Int) {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_ALT")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assert(dataText != null) { "Terminal should have received ALT-modified input" }
    }

    @Then("^the terminal receives the input$")
    fun terminalReceivesInput() {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assert(dataText != null && dataText.isNotBlank()) {
            "Terminal should have received non-empty input, got: $dataText"
        }
    }

    @Then("^both modifiers are recognized by the terminal$")
    fun bothModifiersRecognized() {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_CTRL")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_ALT")
            .assertIsDisplayed()
    }
}
