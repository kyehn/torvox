package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
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

    private fun scrollToBootstrapSection() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = true)
            .performScrollToNode(
                hasTestTag("BootstrapSection"),
            )
        composeTestRule
            .onNodeWithTag("BootstrapSection", useUnmergedTree = true)
            .assertExists()
    }

    @Test
    fun bootstrap_section_exists_in_settings() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapSection", useUnmergedTree = true).assertExists()
    }

    @Test
    fun bootstrap_section_displayed_after_scroll() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapSection", useUnmergedTree = true).assertIsDisplayed()
    }

    @Test
    fun bootstrap_url_field_exists() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapUrlInput").assertExists()
    }

    @Test
    fun bootstrap_url_accepts_text_input() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapUrlInput").performTextClearance()
        composeTestRule.onNodeWithTag("BootstrapUrlInput").performTextInput("https://example.com/bootstrap.zip")
        composeTestRule.onNodeWithTag("BootstrapUrlInput").assertTextContains("https://example.com/bootstrap.zip")
    }

    @Test
    fun bootstrap_install_button_exists() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapInstallButton").assertExists()
    }

    @Test
    fun bootstrap_preset_termux_default_exists() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapPreset_TermuxDefault", useUnmergedTree = true).assertExists()
    }

    @Test
    fun bootstrap_progress_bar_not_displayed_when_not_running() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapProgressBar", useUnmergedTree = true).assertDoesNotExist()
    }

    @Test
    fun bootstrap_result_text_not_displayed_when_not_running() {
        openSettings()
        scrollToBootstrapSection()
        composeTestRule.onNodeWithTag("BootstrapResultText", useUnmergedTree = true).assertDoesNotExist()
    }
}
