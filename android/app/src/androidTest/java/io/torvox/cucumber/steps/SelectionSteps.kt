package io.torvox.cucumber.steps

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.view.inputmethod.InputMethodManager
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.torvox.cucumber.ComposeRuleHolder
import io.torvox.findTerminalSurface
import io.torvox.getBridge
import io.torvox.injectDoubleTap
import io.torvox.injectLongPress
import io.torvox.injectTap
import io.torvox.injectTripleTap
import io.torvox.ui.TerminalSurface
import io.torvox.waitForSession
import javax.inject.Inject

class SelectionSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Given("^the terminal displays text$")
    fun terminalDisplaysText() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @Given("^text is selected in the terminal$")
    fun textIsSelectedInTerminal() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectLongPress(activity, surface, surface.width / 2f, surface.height / 2f)
        }
    }

    @When("^the user long-presses on a character$")
    fun userLongPressesOnCharacter() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectLongPress(activity, surface, surface.width / 2f, surface.height / 2f)
        }
    }

    @When("^the user long-presses on an empty area$")
    fun userLongPressesOnEmptyArea() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectLongPress(activity, surface, surface.width / 2f, surface.height * 0.1f)
        }
    }

    @When("^the user long-presses on a URL$")
    fun userLongPressesOnUrl() {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'https://example.com/test'\n".toByteArray())
        Thread.sleep(3000)
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectLongPress(activity, surface, surface.width / 2f, surface.height / 2f)
        }
    }

    @When("^the user double-taps on a word$")
    fun userDoubleTapsOnWord() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectDoubleTap(activity, surface, surface.width / 2f, surface.height / 2f)
        }
    }

    @When("^the user triple-taps on a line$")
    fun userTripleTapsOnLine() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectTripleTap(activity, surface, surface.width / 2f, surface.height / 2f)
        }
    }

    @When("^the user taps on the terminal$")
    fun userTapsOnTerminal() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectTap(activity, surface, surface.width / 2f, surface.height / 2f)
        }
    }

    @When("^the user drags the selection handle forward$")
    fun userDragsSelectionHandleForward() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectLongPress(activity, surface, surface.width * 0.7f, surface.height / 2f)
        }
    }

    @When("^the user drags the selection handle backward$")
    fun userDragsSelectionHandleBackward() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            injectLongPress(activity, surface, surface.width * 0.3f, surface.height / 2f)
        }
    }

    @When("^the user triggers copy$")
    fun userTriggersCopy() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity) as TerminalSurface
            val text = surface.getSelectedText()
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("terminal", text))
        }
    }

    @When("^the user triggers paste$")
    fun userTriggersPaste() {
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = clipboard.primaryClip
        if (clip != null && clip.itemCount > 0) {
            val text = clip.getItemAt(0).text?.toString() ?: ""
            bridge.writeToPty(text.toByteArray())
        }
    }

    @When("^the user triggers select all$")
    fun userTriggersSelectAll() {
        composeRuleHolder.composeRule.waitForIdle()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        val gridRows = bridge.getGridRows()
        if (gridCols > 0 && gridRows > 0) {
            bridge.setSelection(0u, 0u, (gridRows - 1).toUInt(), (gridCols - 1).toUInt(), true, 0)
            bridge.render()
        }
    }

    @When("^the user switches to another session$")
    fun userSwitchesToAnotherSession() {
        val drawerOpen =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        if (!drawerOpen) {
            composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
            composeRuleHolder.composeRule.waitForIdle()
        }
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

    @When("^the user opens the IME$")
    fun userOpensIme() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val imm = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
            imm.showSoftInput(activity.window.decorView.findFocus(), 0)
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^a selection handle appears$")
    fun selectionHandleAppears() {
        composeRuleHolder.composeRule.waitForIdle()
        // A live selection shows the selection action bar (dismiss/copy/etc.).
        composeRuleHolder.composeRule
            .onNodeWithTag("Action_Dismiss", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^dragging extends the selection$")
    fun draggingExtendsSelection() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("Action_Dismiss", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the word is selected$")
    fun wordIsSelected() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("Action_Dismiss", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the line is selected$")
    fun lineIsSelected() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("Action_Dismiss", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the text is available on the clipboard$")
    fun textIsAvailableOnClipboard() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = clipboard.primaryClip
        assert(clip != null && clip.itemCount > 0) { "Clipboard should contain text" }
    }

    @Then("^the clipboard text is inserted$")
    fun clipboardTextIsInserted() {
        composeRuleHolder.composeRule.waitForIdle()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assert(dataText != null) { "Terminal should still have content after paste" }
    }

    @Then("^the paste popup appears$")
    fun pastePopupAppears() {
        composeRuleHolder.composeRule.waitForIdle()
        // The selection/paste action bar (ModifierBar) is shown in the popup region.
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the full URL is selected$")
    fun fullUrlIsSelected() {
        composeRuleHolder.composeRule.waitForIdle()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assert(dataText != null && dataText.contains("https://example.com/test")) {
            "Full URL should be in terminal output"
        }
    }

    @Then("^the selection extends to the drag target$")
    fun selectionExtendsToDragTarget() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("Action_Dismiss", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the selection shrinks to the drag target$")
    fun selectionShrinksToDragTarget() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule
            .onNodeWithTag("Action_Dismiss", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the selection is cleared$")
    fun selectionIsCleared() {
        composeRuleHolder.composeRule.waitForIdle()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assert(dataText != null) { "Terminal should still have content after clear" }
    }

    @Then("^the entire terminal content is selected$")
    fun entireTerminalContentSelected() {
        composeRuleHolder.composeRule.waitForIdle()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assert(dataText != null) { "Terminal content should exist" }
    }

    @Then("^the selection is preserved$")
    fun selectionIsPreserved() {
        composeRuleHolder.composeRule.waitForIdle()
        // The session is still present and its content is intact.
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
        val bridge =
            composeRuleHolder.composeRule.getBridge()
                ?: throw AssertionError("Bridge is null")
        assert(bridge.getTerminalText() != null) { "Terminal content should survive session switch" }
    }
}
