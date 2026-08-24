package terminal.emulator

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.waitForSession

/**
 * Selection flows through the ViewModel + the system ActionMode menu.
 *
 * The original broadcast-backdoor variants (PARTIAL_SELECT / SELECT_ALL /
 * SHOW_PASTE) were rewritten because instrumentation broadcasts did not
 * reliably reach the dynamically registered TestBackdoorReceivers, and the
 * legacy "SelectionMenuOverlay" compose tag was removed when the menu moved
 * to the system ActionMode toolbar (Termux pattern). The system toolbar is
 * platform UI, so menu items are asserted via UiAutomator (By.text).
 */
class SelectionEspressoTest {
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun startPartialSelection() {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.startSelection(2, 10)
            activity.terminalViewModel.updateSelection(2, 30)
            activity.terminalViewModel.endSelection()
        }
        composeTestRule.waitForIdle()
    }

    @Test
    fun terminalContentIsDisplayed() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun partialSelectShowsSelectionMenu() {
        composeTestRule.waitForSession()
        startPartialSelection()
        // The selection menu is the system ActionMode toolbar (platform
        // text, uppercase by the toolbar style) — visible to UiAutomator.
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        assertTrue("COPY action must be in the selection menu", device.wait(Until.hasObject(By.text("COPY")), 5000))
        assertTrue("SELECT ALL action must be in the selection menu", device.wait(Until.hasObject(By.text("SELECT ALL")), 5000))
    }

    @Test
    fun selectAllShowsSelectionMenu() {
        composeTestRule.waitForSession()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.selectAll(0)
        }
        composeTestRule.waitForIdle()
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        assertTrue("Selection menu must appear after Select All", device.wait(Until.hasObject(By.text("COPY")), 5000))
    }

    @Test
    fun emptyAreaLongPressShowsPasteSelection() {
        composeTestRule.waitForSession()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.showPastePopup(10, 0)
        }
        composeTestRule.waitForIdle()
        // A paste-only selection state (the PasteChipOverlay was removed; an
        // empty-area long-press shows a paste-only selection + menu).
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            assertTrue("Expected a paste-only selection", sel.pasteOnly)
        }
    }

    @Test
    @SuppressLint("DeprecatedCall") // primaryClip: no @Deprecated in API 37; slack-lint rule data lag
    fun copyActionPlacesTextOnClipboard() {
        composeTestRule.waitForSession()
        // Fill a row with text so the copy extracts a non-empty payload.
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.writeToPty("echo 'copy-me-selection-target'\n".toByteArray())
        Thread.sleep(1500)
        startPartialSelection()

        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val copy = device.wait(Until.findObject(By.text("COPY")), 5000)
        assertTrue("COPY action must be present", copy != null)
        requireNotNull(copy).click()
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
        composeTestRule.waitForSession()
        startPartialSelection()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            val start = requireNotNull(sel.start)
            val end = requireNotNull(sel.end)
            assertEquals(2, start.row)
            assertEquals(10, start.col)
            assertEquals(30, end.col)
        }
    }
}