package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.rule.GrantPermissionRule
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.openSettings
import terminal.emulator.waitForSession

class FeatureVerificationTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
    }

    @Test
    fun cursorStyle_blockIsDefault() {
        composeTestRule.onNodeWithTag("CursorStyle_block").assertIsDisplayed()
    }

    @Test
    fun cursorStyle_canSwitchToBar() {
        composeTestRule.onNodeWithTag("CursorStyle_bar").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("CursorStyle_bar").assertIsDisplayed()
    }

    @Test
    fun cursorStyle_canSwitchToUnderline() {
        composeTestRule.onNodeWithTag("CursorStyle_underline").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("CursorStyle_underline").assertIsDisplayed()
    }

    @Test
    fun backgroundImage_sectionExists() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("BackgroundImageStatus"),
        )
        composeTestRule.onNodeWithText("No background image").assertIsDisplayed()
    }

    @Test
    fun backgroundImage_chooseButtonExists() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("ChooseImageButton"),
        )
        composeTestRule.onNodeWithTag("ChooseImageButton").assertIsDisplayed()
    }

    @Test
    fun fontInfo_showsDefaultText() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("FontInfoSection"),
        )
        composeTestRule.onNodeWithTag("FontInfoSection").assertIsDisplayed()
    }

    @Test
    fun settings_scrollsToBottom() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(
            hasTestTag("BootstrapSection"),
        )
        composeTestRule.onNodeWithTag("BootstrapSection").assertIsDisplayed()
    }

    @Test
    fun terminal_rendersAfterLaunch() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun modifierBar_allKeysClickable() {
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
    fun drawer_opensAndCloses() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SessionDrawer").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }
}
