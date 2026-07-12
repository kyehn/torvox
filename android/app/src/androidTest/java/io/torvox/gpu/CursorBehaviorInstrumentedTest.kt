package io.torvox.gpu

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTextInput
import io.torvox.MainActivity
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class CursorBehaviorInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun cursorBlink_disabled_terminalStaysVisible() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.setCursorBlinkEnabled(false)
        bridge.resetCursorBlink()
        Thread.sleep(2000)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorBlink_enabled_terminalStaysVisibleAfterBlinks() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.setCursorBlinkEnabled(true)
        bridge.setCursorBlinkSpeedMs(200)
        bridge.resetCursorBlink()
        Thread.sleep(600)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorVisibility_persistsAfterTyping() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.setCursorBlinkEnabled(false)
        bridge.resetCursorBlink()
        composeTestRule.onNodeWithTag("TerminalScreen").performTextInput("echo test")
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorNeverRandomlyDisappears() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.setCursorBlinkEnabled(false)
        bridge.resetCursorBlink()
        for (i in 0..9) {
            Thread.sleep(500)
            composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        }
    }

    @Test
    fun cursorShape_switchRoundTrip() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.setCursorStyle("bar")
        Thread.sleep(100)
        bridge.setCursorStyle("underline")
        Thread.sleep(100)
        bridge.setCursorStyle("block")
        Thread.sleep(100)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorBlink_speedChange_doesNotCrash() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        bridge.setCursorBlinkEnabled(true)
        bridge.setCursorBlinkSpeedMs(100)
        Thread.sleep(50)
        bridge.setCursorBlinkSpeedMs(530)
        Thread.sleep(50)
        bridge.setCursorBlinkSpeedMs(1000)
        Thread.sleep(50)
        bridge.resetCursorBlink()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
