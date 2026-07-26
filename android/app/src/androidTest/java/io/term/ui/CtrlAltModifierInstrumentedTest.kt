package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.term.MainActivity
import org.junit.Rule
import org.junit.Test

class CtrlAltModifierInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun ctrl_button_displayed() {
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
    }

    @Test
    fun alt_button_displayed() {
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
    }

    @Test
    fun ctrl_button_clickable() {
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
    }

    @Test
    fun alt_button_clickable() {
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
    }

    @Test
    fun ctrl_click_does_not_crash() {
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun alt_click_does_not_crash() {
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun ctrl_then_esc_does_not_crash() {
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_ESC").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun rapid_ctrl_clicks_do_not_crash() {
        for (i in 1..10) {
            composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun rapid_alt_clicks_do_not_crash() {
        for (i in 1..10) {
            composeTestRule.onNodeWithTag("Key_ALT").performClick()
        }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
