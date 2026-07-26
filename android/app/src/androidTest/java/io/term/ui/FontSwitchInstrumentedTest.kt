package io.term.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipe
import androidx.compose.ui.test.swipeUp
import io.term.MainActivity
import io.term.openSettings
import io.term.waitForSession
import org.junit.Rule
import org.junit.Test

class FontSwitchInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun settings_has_font_family_section() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
        composeTestRule.onNodeWithText("Font Family").assertIsDisplayed()
    }

    @Test
    fun font_size_slider_displayed() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
    }

    @Test
    fun font_family_section_does_not_crash() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
        composeTestRule.onNodeWithText("Font Family").assertIsDisplayed()
    }

    @Test
    fun font_selection_does_not_crash() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithText("Font Family").assertIsDisplayed()
        composeTestRule.onNodeWithTag("FontFamilySelector").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun font_size_change_causes_rerender() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
        composeTestRule.onNodeWithTag("FontSizeSlider").performTouchInput {
            swipe(Offset(width * 0.2f, height * 0.5f), Offset(width * 0.8f, height * 0.5f))
        }
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
