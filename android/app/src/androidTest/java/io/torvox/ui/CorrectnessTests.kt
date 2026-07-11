package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.torvox.MainActivity
import io.torvox.openDrawer
import io.torvox.waitForSession
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class CorrectnessTests {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun test01_terminal_screen_renders() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun test02_modifier_bar_visible_with_all_keys() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_TAB").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_PGUP").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_PGDN").assertIsDisplayed()
    }

    @Test
    fun test03_drawer_shows_sessions_and_settings() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsButton").assertIsDisplayed()
        composeTestRule.onNodeWithText("Session 1").assertIsDisplayed()
    }

    @Test
    fun test04_search_bar_appears_after_opening() {
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
    fun test05_terminal_still_visible_after_search_close() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun test06_settings_screen_opens() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun test07_font_family_selector_in_settings() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("FontFamilySelector").assertIsDisplayed()
    }

    @Test
    fun test08_settings_back_shows_terminal() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun test10_scrollback_swipe_still_renders() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalContent").performTouchInput { swipeUp() }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun test11_new_session_button_in_drawer() {
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
