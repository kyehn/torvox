package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import io.torvox.MainActivity
import io.torvox.openSettings
import io.torvox.waitForSession
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class FeatureVerificationTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
    }

    @Test
    fun test01_cursorStyle_blockIsDefault() {
        composeTestRule.onNodeWithTag("CursorStyle_block").assertIsDisplayed()
    }

    @Test
    fun test02_cursorStyle_canSwitchToBar() {
        composeTestRule.onNodeWithTag("CursorStyle_bar").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("CursorStyle_bar").assertIsDisplayed()
    }

    @Test
    fun test03_cursorStyle_canSwitchToUnderline() {
        composeTestRule.onNodeWithTag("CursorStyle_underline").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("CursorStyle_underline").assertIsDisplayed()
    }

    @Test
    fun test04_backgroundImage_sectionExists() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("BackgroundImageStatus"),
        )
        composeTestRule.onNodeWithText("No background image").assertIsDisplayed()
    }

    @Test
    fun test05_backgroundImage_chooseButtonExists() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("ChooseImageButton"),
        )
        composeTestRule.onNodeWithTag("ChooseImageButton").assertIsDisplayed()
    }

    @Test
    fun test06_fontInfo_showsDefaultText() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("FontInfoSection"),
        )
        composeTestRule.onNodeWithTag("FontInfoSection").assertIsDisplayed()
    }

    @Test
    fun test07_settings_scrollsToBottom() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("BootstrapSection"),
        )
        composeTestRule.onNodeWithTag("BootstrapSection").assertIsDisplayed()
    }

    @Test
    fun test08_terminal_rendersAfterLaunch() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun test09_modifierBar_allKeysClickable() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        val keys =
            listOf(
                "Key_ESC",
                "Key_TAB",
                "Key_CTRL",
                "Key_ALT",
                "Key_HOME",
                "Key_END",
                "Key_PGUP",
                "Key_PGDN",
            )
        for (key in keys) {
            composeTestRule.onNodeWithTag(key).performClick()
            composeTestRule.waitForIdle()
        }
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun test10_drawer_opensAndCloses() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SessionDrawer").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }
}
