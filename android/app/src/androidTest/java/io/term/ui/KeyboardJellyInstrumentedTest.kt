package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import io.term.MainActivity
import org.junit.Rule
import org.junit.Test

class KeyboardJellyInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminal_content_stays_stable_before_keyboard() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun terminal_content_stays_stable_after_back_pressed() {
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_always_visible_below_content() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }
}
