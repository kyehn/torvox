package io.term.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.captureRoboImage
import io.term.MainActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalComposableRoborazziTest {
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
