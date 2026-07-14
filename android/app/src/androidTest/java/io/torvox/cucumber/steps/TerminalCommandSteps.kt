package io.torvox.cucumber.steps

import androidx.compose.ui.test.hasTestTag
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.torvox.cucumber.ComposeRuleHolder
import io.torvox.getBridge
import io.torvox.waitForSession
import javax.inject.Inject

class TerminalCommandSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the user types \"([^\"]+)\" and presses Enter$")
    fun userTypesAndPressesEnter(command: String) {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("$command\n".toByteArray())
        Thread.sleep(3000)
    }

    @When("^the user runs \"([^\"]+)\"$")
    fun userRunsCommand(command: String) {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("$command\n".toByteArray())
        // Force an immediate render pass so the queued user write is processed.
        // The render loop will pick up the PTY output on its next cycle.
        val renderResult = bridge.render()
        if (renderResult < 0) {
            android.util.Log.w("Cucumber", "userRunsCommand: render returned $renderResult")
        }
        Thread.sleep(2000)
    }

    @Then("^the output appears on the terminal screen$")
    fun outputAppearsOnTerminalScreen() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 10000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        // Force a render pass to drain PTY output that arrived after the last
        // render call.  This is necessary because the render loop may have
        // exited before the shell had a chance to respond.
        bridge.render()
        val dataText = bridge.getTerminalText()
        assert(dataText != null && dataText.contains("HELLO_TORVOX")) {
            "Expected HELLO_TORVOX in output, got: $dataText"
        }
    }

    @Then("^the output contains \"([^\"]+)\" or \"([^\"]+)\"$")
    fun outputContainsEither(
        expected1: String,
        expected2: String,
    ) {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 15000) {
            bridge.render()
            val dataText = bridge.getTerminalText()
            dataText != null && (dataText.contains(expected1) || dataText.contains(expected2))
        }
    }

    @Then("^all three outputs are visible in the terminal$")
    fun allThreeOutputsVisible() {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 20000) {
            bridge.render()
            val text = bridge.getTerminalText()
            text != null && text.contains("first") && text.contains("second") && text.contains("third")
        }
    }
}
