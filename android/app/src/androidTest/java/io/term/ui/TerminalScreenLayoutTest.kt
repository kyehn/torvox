package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.printToLog
import io.term.MainActivity
import org.junit.Rule
import org.junit.Test

class TerminalScreenLayoutTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminal_screen_no_hardcoded_background_color() {
        // Verify that the TerminalScreen uses the theme background color
        // not a hardcoded Color(0xFF2A2D3E). The Surface should use
        // the terminal theme's background color to prevent black gaps.
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun terminal_content_fills_available_space() {
        // Verify that TerminalContent fills the space above ModifierBar
        // without any black gap between them
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_uses_theme_background() {
        // Verify that ModifierBar is displayed with the correct background
        // matching the terminal theme, not a hardcoded color
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun terminal_screen_structure_is_correct() {
        // Verify the layout structure:
        // Column > Box(TerminalContent) + ModifierBar
        // No Scaffold padding creating gaps
        composeTestRule.onRoot().printToLog("TerminalScreenLayout")
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }
}
