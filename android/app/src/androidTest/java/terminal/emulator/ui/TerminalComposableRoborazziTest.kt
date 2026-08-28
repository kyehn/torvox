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

@RunWith(AndroidJUnit4::class)
class TerminalComposableRoborazziTest {
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
    fun terminalScreen_screenshot_captured() {
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun modifierBar_screenshot_captured() {
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("ModifierBar")
            .captureRoboImage()
    }

    @Test
    fun terminalContent_screenshot_captured() {
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("TerminalContent")
            .captureRoboImage()
    }
}
