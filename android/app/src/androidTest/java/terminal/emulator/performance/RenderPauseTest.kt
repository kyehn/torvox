package terminal.emulator.performance

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.rule.GrantPermissionRule
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.openSettings
import terminal.emulator.waitForSession

class RenderPauseTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun settings_open_does_not_crash() {
        composeTestRule.openSettings()
        // Verify settings screen rendered: look for back button or settings UI
        composeTestRule.onNodeWithTag("SettingsBackButton").assertExists()
    }

    @Test
    fun settings_close_resumes_rendering() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        // After closing settings, the terminal screen must be visible again
        // and the session must still be alive (render loop resumed).
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        composeTestRule.waitForSession()
    }
}
