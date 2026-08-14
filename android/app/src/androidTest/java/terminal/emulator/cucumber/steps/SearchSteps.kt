package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import javax.inject.Inject

class SearchSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Given("^a terminal session is active$")
    fun terminalSessionIsActive() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen")
            .assertIsDisplayed()
    }

    @Given("^the terminal has search highlights active$")
    fun terminalHasSearchHighlightsActive() {
        // Open search bar and search for something common
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchButton")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performClick()
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performTextInput("the")
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Given("^the search bar is visible$")
    fun searchBarIsVisible() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchButton")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("TextSearchBar")
            .assertIsDisplayed()
    }

    @When("^the user opens the search bar from the session panel$")
    fun userOpensSearchBar() {
        val composeRule = composeRuleHolder.composeRule

        // Tap SearchButton directly (ModalNavigationDrawer composes drawer content even when closed)
        composeRule
            .onNodeWithTag("SearchButton")
            .performClick()
        composeRule.waitForIdle()

        // After SearchButton click, handle the drawer close coroutine launch timing
        // The onClose launches a coroutine; wait for animations
        composeRule.waitForIdle()
    }

    @When("^the user closes the search bar$")
    fun userClosesSearchBar() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchClose")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the soft keyboard opens$")
    fun softKeyboardOpens() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the search bar is displayed at the bottom$")
    fun searchBarIsDisplayedAtBottom() {
        val composeRule = composeRuleHolder.composeRule
        composeRule.waitForIdle()

        // The search bar is at the bottom; check its internal nodes
        composeRule
            .onNodeWithTag("SearchTextField")
            .assertIsDisplayed()
        composeRule
            .onNodeWithTag("SearchClose")
            .assertIsDisplayed()
    }

    @Then("^the modifier bar is hidden$")
    fun modifierBarIsHidden() {
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar")
            .assertIsNotDisplayed()
    }

    @Then("^the modifier bar is visible again$")
    fun modifierBarIsVisibleAgain() {
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar")
            .assertIsDisplayed()
    }

    @Then("^all search highlights disappear$")
    fun allSearchHighlightsDisappear() {
        composeRuleHolder.composeRule.waitForIdle()
        // Closing the search bar removes the result counter / highlight UI.
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchResultCount")
            .assertIsNotDisplayed()
    }

    @Then("^the search bar remains visible above the keyboard$")
    fun searchBarRemainsVisibleAboveKeyboard() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .assertIsDisplayed()
    }
}
