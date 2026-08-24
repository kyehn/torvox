package terminal.emulator

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for the text-selection feature.
 *
 * Selection is driven through the existing broadcast intents
 * (terminal.emulator.PARTIAL_SELECT / SELECT_ALL / SHOW_PASTE) so the tests do not depend on
 * the GPU render thread or the emulator's long-press timing. The tests then assert that
 * the selection is active and that the Copy action lands the selected text on the clipboard.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class SelectionEspressoTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun sendSelectionBroadcast(
        action: String,
        extras: Intent.() -> Unit = {},
    ) {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val intent = Intent(action).apply(extras)
            activity.sendBroadcast(intent)
        }
        composeTestRule.waitForIdle()
    }

    @Test
    fun terminalContentIsDisplayed() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun partialSelectShowsSelectionMenu() {
        sendSelectionBroadcast("terminal.emulator.PARTIAL_SELECT") {
            putExtra("startRow", 0)
            putExtra("startCol", 0)
            putExtra("endRow", 0)
            putExtra("endCol", 10)
        }
        composeTestRule.onNodeWithTag("SelectionMenuOverlay").assertIsDisplayed()
        // Menu must include the Copy / Select All / Paste actions.
        composeTestRule.onNodeWithText("Copy").assertIsDisplayed()
        composeTestRule.onNodeWithText("Select All").assertIsDisplayed()
        composeTestRule.onNodeWithText("Paste").assertIsDisplayed()
    }

    @Test
    fun selectAllShowsSelectionMenu() {
        sendSelectionBroadcast("terminal.emulator.SELECT_ALL")
        composeTestRule.onNodeWithTag("SelectionMenuOverlay").assertIsDisplayed()
        composeTestRule.onNodeWithText("Select All").assertIsDisplayed()
    }

    @Test
    fun emptyAreaLongPressShowsPasteSelection() {
        sendSelectionBroadcast("terminal.emulator.SHOW_PASTE") {
            putExtra("row", 10)
            putExtra("col", 0)
        }
        // the PasteChipOverlay was removed; an empty-area
        // long-press now shows a paste-only selection state.
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            assertTrue("Expected a paste-only selection", sel.pasteOnly)
        }
    }

    @Test
    @org.junit.Ignore("Requires the native data path: scrollbackLine is an implemented (native query path is wired), so extractSelectedText yields an empty string and Copy never writes the clipboard  — native is wired, but the continuous render loop makes Compose idling time out on software-rendered emulators; needs a hardware-accelerated device")
    @SuppressLint("DeprecatedCall") // primaryClip: no @Deprecated in API 37; slack-lint rule data lag
    fun copyActionPlacesTextOnClipboard() {
        // Select a known range, then click Copy and verify the clipboard.
        sendSelectionBroadcast("terminal.emulator.PARTIAL_SELECT") {
            putExtra("startRow", 0)
            putExtra("startCol", 0)
            putExtra("endRow", 0)
            putExtra("endCol", 10)
        }
        composeTestRule.onNodeWithTag("SelectionMenuOverlay").assertIsDisplayed()
        composeTestRule.onNodeWithText("Copy").performClick()
        composeTestRule.waitForIdle()

        composeTestRule.activityRule.scenario.onActivity { activity ->
            val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clip = clipboard.primaryClip
            assertTrue("Clipboard should contain a clip after Copy", clip != null)
            val text =
                clip
                    ?.getItemAt(0)
                    ?.text
                    ?.toString()
                    .orEmpty()
            assertFalse("Clipboard text should not be empty after Copy", text.isEmpty())
        }
    }

    @Test
    fun selectionStateIsActiveAfterPartialSelect() {
        sendSelectionBroadcast("terminal.emulator.PARTIAL_SELECT") {
            putExtra("startRow", 1)
            putExtra("startCol", 2)
            putExtra("endRow", 3)
            putExtra("endCol", 8)
        }
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            val start = requireNotNull(sel.start)
            val end = requireNotNull(sel.end)
            assertEquals(1, start.row)
            assertEquals(2, start.col)
            assertEquals(3, end.row)
            assertEquals(8, end.col)
        }
    }
}
