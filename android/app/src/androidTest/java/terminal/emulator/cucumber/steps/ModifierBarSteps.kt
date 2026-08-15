package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.waitForSession
import javax.inject.Inject

class ModifierBarSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Then("^the modifier bar shows ESC, TAB, CTRL, ALT, HOME, END, PGUP, PGDN keys$")
    fun modifierBarShowsAllKeys() {
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_ESC")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_TAB")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_CTRL")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_ALT")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_HOME")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_END")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_PGUP")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_PGDN")
            .assertIsDisplayed()
    }

    @When("^the CTRL key is tapped$")
    fun ctrlKeyIsTapped() {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_CTRL")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the CTRL key is tapped twice$")
    fun ctrlKeyIsTappedTwice() {
        val rule = composeRuleHolder.composeRule
        rule.onNodeWithTag("Key_CTRL").performClick()
        rule.onNodeWithTag("Key_CTRL").performClick()
        rule.waitForIdle()
    }

    @Then("^the CTRL key toggles appearance$")
    fun ctrlKeyTogglesAppearance() {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_CTRL")
            .assertIsSelected()
    }

    @Then("^the CTRL key returns to default appearance$")
    fun ctrlKeyReturnsToDefaultAppearance() {
        composeRuleHolder.composeRule
            .onNodeWithTag("Key_CTRL")
            .assertIsNotSelected()
    }
}
