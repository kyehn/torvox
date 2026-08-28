package terminal.emulator.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.GrantPermissionRule
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.waitForSession

@RunWith(AndroidJUnit4::class)
class TerminalScreenRoborazziTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val roborazziRule =
        RoborazziRule(
            options =
            RoborazziRule.Options(
                roborazziOptions =
                RoborazziOptions(
                    compareOptions =
                    RoborazziOptions.CompareOptions(
                        changeThreshold = 0.05f,
                    ),
                ),
            ),
        )

    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminalScreen_light_matchesBaseline() { // B1: SGR colors / styles rendered on screen vs golden
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").captureRoboImage()
    }

    @Test
    fun terminalScreen_darkTheme_matchesBaseline() { // B14: inverse / dark-theme coloring vs golden
        composeTestRule.waitForSession()
        val uiModeManager = composeTestRule.activity.getSystemService(android.app.UiModeManager::class.java)
        uiModeManager.nightMode = android.app.UiModeManager.MODE_NIGHT_YES
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").captureRoboImage()
        uiModeManager.nightMode = android.app.UiModeManager.MODE_NIGHT_NO
        composeTestRule.waitForIdle()
    }
}
