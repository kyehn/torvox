package io.torvox

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
 * (io.torvox.PARTIAL_SELECT / SELECT_ALL / SHOW_PASTE) so the tests do not depend on
 * the GPU render thread or the emulator's long-press timing. The tests then assert that
 * the Compose overlays (SelectionMenuOverlay / PasteChipOverlay) are shown and that the
 * Copy action lands the selected text on the clipboard.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class SelectionEspressoTest {
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
        sendSelectionBroadcast("io.torvox.PARTIAL_SELECT") {
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
        sendSelectionBroadcast("io.torvox.SELECT_ALL")
        composeTestRule.onNodeWithTag("SelectionMenuOverlay").assertIsDisplayed()
        composeTestRule.onNodeWithText("Select All").assertIsDisplayed()
    }

    @Test
    fun emptyAreaLongPressShowsPasteChip() {
        sendSelectionBroadcast("io.torvox.SHOW_PASTE") {
            putExtra("row", 10)
            putExtra("col", 0)
        }
        composeTestRule.onNodeWithTag("PasteChipOverlay").assertIsDisplayed()
        composeTestRule.onNodeWithText("Paste").assertIsDisplayed()
    }

    @Test
    fun copyActionPlacesTextOnClipboard() {
        // Select a known range, then click Copy and verify the clipboard.
        sendSelectionBroadcast("io.torvox.PARTIAL_SELECT") {
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
        sendSelectionBroadcast("io.torvox.PARTIAL_SELECT") {
            putExtra("startRow", 1)
            putExtra("startCol", 2)
            putExtra("endRow", 3)
            putExtra("endCol", 8)
        }
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            assertEquals(1, sel.start!!.row)
            assertEquals(2, sel.start!!.col)
            assertEquals(3, sel.end!!.row)
            assertEquals(8, sel.end!!.col)
        }
    }
}
