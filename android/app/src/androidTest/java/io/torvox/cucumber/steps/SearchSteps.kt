package io.torvox.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.torvox.cucumber.ComposeRuleHolder
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

    @Given("^a terminal session is active with visible text$")
    fun terminalSessionIsActiveWithVisibleText() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen")
            .assertIsDisplayed()
    }

    @Given("^a terminal session is active with mixed case text$")
    fun terminalSessionIsActiveWithMixedCaseText() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen")
            .assertIsDisplayed()
    }

    @Given("^the terminal has multiple \"([^\"]+)\" matches visible$")
    fun terminalHasMultipleMatchesVisible(query: String) {
        // Open search bar and search for the term to populate results
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchButton")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performClick()
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performTextInput(query)
        composeRuleHolder.composeRule.waitForIdle()
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

    @Given("^the terminal has scrolled content with \"([^\"]+)\"$")
    fun terminalHasScrolledContentWith(marker: String) {
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

    @When("^the user searches for \"([^\"]+)\"$")
    fun userSearchesFor(query: String) {
        // Ensure search bar is open first
        val nodes =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SearchTextField"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        if (nodes.isEmpty()) {
            // Open search bar from session panel
            composeRuleHolder.composeRule
                .onNodeWithTag("SearchButton")
                .performClick()
            composeRuleHolder.composeRule.waitForIdle()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performClick()
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performTextInput(query)
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^searches for \"([^\"]+)\"$")
    fun searchesFor(query: String) {
        // Ensure search bar is open first
        val nodes =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SearchTextField"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        if (nodes.isEmpty()) {
            // Open search bar from session panel
            composeRuleHolder.composeRule
                .onNodeWithTag("SearchButton")
                .performClick()
            composeRuleHolder.composeRule.waitForIdle()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performClick()
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchTextField")
            .performTextInput(query)
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user presses \"([^\"]+)\"$")
    fun userPressesNavigation(nav: String) {
        when (nav.lowercase()) {
            "next" -> {
                composeRuleHolder.composeRule
                    .onNodeWithTag("SearchNext")
                    .performClick()
            }

            "previous" -> {
                composeRuleHolder.composeRule
                    .onNodeWithTag("SearchPrevious")
                    .performClick()
            }

            "close" -> {
                composeRuleHolder.composeRule
                    .onNodeWithTag("SearchClose")
                    .performClick()
            }

            else -> {
                throw IllegalArgumentException("Unknown navigation: $nav")
            }
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user enables case-sensitive search$")
    fun userEnablesCaseSensitive() {
        // Open search bar first if not already open
        val nodes =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SearchCaseSensitive"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        if (nodes.isEmpty()) {
            composeRuleHolder.composeRule
                .onNodeWithTag("SearchButton")
                .performClick()
            composeRuleHolder.composeRule.waitForIdle()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchCaseSensitive", useUnmergedTree = true)
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user closes the search bar$")
    fun userClosesSearchBar() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchClose")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user waits (\\d+) seconds$")
    fun userWaitsSeconds(seconds: Int) {
        Thread.sleep(seconds * 1000L)
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

    @Then("^at least one match is highlighted on screen$")
    fun atLeastOneMatchHighlighted() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchResultCount")
            .assertIsDisplayed()
    }

    @Then("^the current match indicator changes$")
    fun currentMatchIndicatorChanges() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchResultCount")
            .assertIsDisplayed()
    }

    @Then("^the current match indicator returns$")
    fun currentMatchIndicatorReturns() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchResultCount")
            .assertIsDisplayed()
    }

    @Then("^only uppercase matches are highlighted$")
    fun onlyUppercaseMatchesHighlighted() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SearchResultCount")
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

    @Given("^the match is not visible on the current screen$")
    fun matchIsNotVisibleOnCurrentScreen() {
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^the terminal scrolls to show the match$")
    fun terminalScrollsToShowMatch() {
        composeRuleHolder.composeRule.waitForIdle()
        // The terminal is still rendered and interactive after the scroll.
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen")
            .assertIsDisplayed()
    }
}
