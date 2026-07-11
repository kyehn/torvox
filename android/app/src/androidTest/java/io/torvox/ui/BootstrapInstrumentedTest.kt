package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.torvox.MainActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BootstrapInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun openSettings() {
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
    }

    @Test
    fun bootstrap_section_exists_in_settings() {
        openSettings()
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("BootstrapSection"),
        )
        composeTestRule.onNodeWithTag("BootstrapSection").assertExists()
    }

    @Test
    fun bootstrap_section_displayed_after_scroll() {
        openSettings()
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("BootstrapSection"),
        )
        composeTestRule.onNodeWithTag("BootstrapSection").assertIsDisplayed()
    }
}
