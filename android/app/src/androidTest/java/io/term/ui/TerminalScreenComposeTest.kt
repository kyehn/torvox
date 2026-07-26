package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.term.MainActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalScreenComposeTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminalScreen_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun terminalContent_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun modifierBar_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun modifierBar_escKey_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
    }

    @Test
    fun modifierBar_ctrlKey_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
    }

    @Test
    fun modifierBar_altKey_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
    }

    @Test
    fun modifierBar_tabKey_isDisplayed() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_TAB").assertIsDisplayed()
    }

    @Test
    fun drawer_button_is_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").assertIsDisplayed()
    }
}
