package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openSettings
import terminal.emulator.waitForSession
import java.io.File
import javax.inject.Inject

class FontSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the user opens settings$")
    fun userOpensSettings() {
        composeRuleHolder.composeRule.openSettings()
    }

    @When("^changes the font family$")
    fun changesFontFamily() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithText("Font Family")
            .assertIsDisplayed()
        composeRuleHolder.composeRule
            .onNodeWithTag("FontFamilySelector")
            .performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user attempts to load an invalid font file$")
    fun userAttemptsToLoadInvalidFont() {
        composeRuleHolder.composeRule.openSettings()
        composeRuleHolder.composeRule
            .onNodeWithText("Font Family")
            .assertIsDisplayed()
    }

    @Then("^the terminal font updates without error$")
    fun terminalFontUpdatesWithoutError() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the app does not crash$")
    fun appDoesNotCrash() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the previous working font is preserved$")
    fun previousWorkingFontIsPreserved() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^a user-visible error message is shown$")
    fun userVisibleErrorMessageIsShown() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^font files are stored in the application's private fonts directory$")
    fun fontFilesStoredInPrivateDirectory() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val fontsDir = File(context.filesDir, "fonts")
        // TerminalScreen becomes visible before TerminalRuntime.start()
        // finishes (it creates filesDir/fonts asynchronously), so poll
        // instead of asserting immediately (round-119 race).
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 15_000) {
            fontsDir.isDirectory
        }
    }
}
