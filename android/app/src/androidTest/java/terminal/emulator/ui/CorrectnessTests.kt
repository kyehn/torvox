
package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession

@RunWith(AndroidJUnit4::class)
class CorrectnessTests {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminal_screen_renders() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_visible_with_all_keys() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_TAB").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_PGUP").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_PGDN").assertIsDisplayed()
    }

    @Test
    fun drawer_shows_sessions_and_settings() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsButton").assertIsDisplayed()
        composeTestRule.onNodeWithText("Session 1").assertIsDisplayed()
    }

    @Test
    fun search_bar_appears_after_opening() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("test")
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchResultCount").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TextSearchBar").assertIsDisplayed()
    }

    @Test
    fun terminal_still_visible_after_search_close() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun settings_screen_opens() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun font_family_selector_in_settings() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("FontFamilySelector").assertIsDisplayed()
    }

    @Test
    fun settings_back_shows_terminal() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun scrollback_swipe_still_renders() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalContent").performTouchInput { swipeUp() }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun new_session_button_in_drawer() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
