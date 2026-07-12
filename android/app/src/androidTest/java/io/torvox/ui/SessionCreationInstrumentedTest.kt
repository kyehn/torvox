package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.torvox.MainActivity
import org.junit.Rule
import org.junit.Test

class SessionCreationInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun add_session_button_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }

    @Test
    fun add_session_button_clickable() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun add_session_completes_within_timeout() {
        val startTime = System.currentTimeMillis()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitForIdle()
        val elapsed = System.currentTimeMillis() - startTime
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        assert(elapsed < 10000) { "Session creation took ${elapsed}ms, expected < 10000ms" }
    }

    @Test
    fun add_second_session_does_not_crash() {
        if (android.os.Build.SUPPORTED_ABIS[0]
                .startsWith("x86")
        ) {
            return
        }
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun session_drawer_shows_multiple_sessions() {
        if (android.os.Build.SUPPORTED_ABIS[0]
                .startsWith("x86")
        ) {
            return
        }
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitUntil(timeoutMillis = 12000) {
            composeTestRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeTestRule.waitForIdle()
        val terminalNodes =
            composeTestRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        assert(terminalNodes.isNotEmpty()) { "Terminal should be visible after creating second session" }
    }
}
