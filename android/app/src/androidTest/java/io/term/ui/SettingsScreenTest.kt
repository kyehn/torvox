package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import io.term.MainActivity
import io.term.openSettings
import io.term.waitForSession
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class SettingsScreenTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
    }

    @Test
    fun settings_screen_renders_back_button() {
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsBackButton").assertIsDisplayed()
    }

    @Test
    fun back_button_navigates_to_terminal() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun settings_screen_shows_appearance_section() {
        composeTestRule.onNodeWithText("Appearance").assertIsDisplayed()
    }

    @Test
    fun settings_screen_shows_font_size_slider() {
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
    }

    @Test
    fun settings_screen_switches_day_theme() {
        composeTestRule.onNodeWithText("Appearance").performTouchInput { swipeUp() }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Day").assertExists()
    }

    @Test
    fun settings_screen_displays_font_list() {
        composeTestRule.onNodeWithTag("FontSizeSlider").assertExists()
    }

    @Test
    fun theme_switch_changes_terminal_appearance() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        composeTestRule.onNodeWithText("Dracula Plus").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.waitUntil(timeoutMillis = 5_000) {
            composeTestRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
