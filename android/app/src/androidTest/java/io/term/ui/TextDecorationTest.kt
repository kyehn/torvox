package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import io.term.MainActivity
import io.term.openSettings
import io.term.waitForSession
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class TextDecorationTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
    }

    @Test
    fun settings_shows_cursor_style_options() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performTouchInput { swipeUp() }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Cursor Style").assertExists()
        composeTestRule.onNodeWithText("Block").assertIsDisplayed()
    }

    @Test
    fun settings_shows_font_options() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performTouchInput { swipeUp() }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Font Size").assertExists()
    }
}
