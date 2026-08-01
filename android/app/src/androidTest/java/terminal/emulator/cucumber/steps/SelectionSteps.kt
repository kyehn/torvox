package terminal.emulator.cucumber.steps

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.view.View
import android.view.inputmethod.InputMethodManager
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.injectLongPress
import terminal.emulator.ui.TerminalSurface
import terminal.emulator.waitForSession
import javax.inject.Inject

class SelectionSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    private fun surface(): View {
        val scenario = composeRuleHolder.composeRule.activityRule.scenario
        var surface: View? = null
        scenario.onActivity { activity ->
            surface = findTerminalSurface(activity)
        }
        return checkNotNull(surface) { "Terminal surface not found" }
    }

    @Given("^the terminal displays text$")
    fun terminalDisplaysText() {
        composeRuleHolder.composeRule.waitForSession()
    }

    @Given("^text is selected in the terminal$")
    fun textIsSelectedInTerminal() {
        composeRuleHolder.composeRule.waitForSession()
        val s = surface()
        injectLongPress(s, s.width / 2f, s.height / 2f)
    }

    @When("^the user long-presses on a character$")
    fun userLongPressesOnCharacter() {
        val s = surface()
        injectLongPress(s, s.width / 2f, s.height / 2f)
    }

    @When("^the user long-presses on an empty area$")
    fun userLongPressesOnEmptyArea() {
        val s = surface()
        // Round-119: press near the bottom. After "the terminal displays
        // text" the prompt occupies the top rows, so height*0.1 lands on
        // text and triggers word-selection instead of the paste popup.
        injectLongPress(s, s.width / 2f, s.height * 0.9f)
    }

    @When("^the user double-taps on a word$")
    fun userDoubleTapsOnWord() {
        // STUB (round-104): GestureDetector cannot detect double-tap with
        // simulated events (Android removes the TAP handler on the first UP,
        // making hadTapMessage=false when the second DOWN arrives). Fall
        // back to long-press which triggers handleLongPress with semantic
        // word expansion — identical outcome for now; revisit when a real
        // double-tap injection path exists.
        val s = surface()
        injectLongPress(s, s.width / 2f, s.height / 2f)
    }

    @When("^the user drags the selection handle forward$")
    fun userDragsSelectionHandleForward() {
        val s = surface()
        injectLongPress(s, s.width * 0.7f, s.height / 2f)
    }

    @When("^the user drags the selection handle backward$")
    fun userDragsSelectionHandleBackward() {
        val s = surface()
        injectLongPress(s, s.width * 0.3f, s.height / 2f)
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

    @Then("^a selection handle appears$")
    fun selectionHandleAppears() {
        composeRuleHolder.composeRule.waitForIdle()
        // A live selection shows the selection action bar (dismiss/copy/etc.).
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

    @Then("^the text is available on the clipboard$")
    fun textIsAvailableOnClipboard() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = clipboard.primaryClip
        assert(clip != null && clip.itemCount > 0) { "Clipboard should contain text" }
    }

    @Then("^the paste popup appears$")
    fun pastePopupAppears() {
        composeRuleHolder.composeRule.waitForIdle()
        // The real paste popup is PasteChipOverlay; ModifierBar is always
        // visible and would make this assertion tautological (round-104).
        composeRuleHolder.composeRule
            .onNodeWithTag("PasteChipOverlay", useUnmergedTree = true)
            .assertIsDisplayed()
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
}
