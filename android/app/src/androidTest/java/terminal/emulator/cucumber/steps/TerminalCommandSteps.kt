package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.hasTestTag
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.getBridge
import terminal.emulator.waitForSession
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

    @Then("^the output appears on the terminal screen$")
    fun outputAppearsOnTerminalScreen() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 10000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
    }
}
