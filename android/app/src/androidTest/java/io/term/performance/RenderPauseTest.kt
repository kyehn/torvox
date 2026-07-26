package io.term.performance

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.term.MainActivity
import io.term.openSettings
import io.term.waitForSession
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class RenderPauseTest {
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
        composeTestRule.onNodeWithTag("settings_back_button").assertExists()
    }

    @Test
    fun settings_close_resumes_rendering() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("settings_back_button").performClick()
        // After closing settings, terminal should still render
        composeTestRule.waitForSession()
    }
}
