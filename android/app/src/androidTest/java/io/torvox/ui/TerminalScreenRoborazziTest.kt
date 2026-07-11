package io.torvox.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.captureRoboImage
import io.torvox.MainActivity
import io.torvox.waitForSession
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalScreenRoborazziTest {
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
