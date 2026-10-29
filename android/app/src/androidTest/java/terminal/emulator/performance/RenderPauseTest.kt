package terminal.emulator.performance

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import org.junit.After
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.MainActivity
import terminal.emulator.closeSettingsOverlay
import terminal.emulator.openSettings
import terminal.emulator.waitForSession

class RenderPauseTest : ComposeTerminalLogcatTest() {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.

    @After
    fun resetScreen() = composeTestRule.closeSettingsOverlay()

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
