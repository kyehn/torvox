package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import io.torvox.MainActivity
import io.torvox.bridge.TorvoxBridge
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class CursorBehaviorInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private var bridge: TorvoxBridge? = null

    private fun getBridgeOrWait(timeoutMs: Long = 15_000): TorvoxBridge {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            bridge = composeTestRule.getBridge()
            if (bridge != null) return bridge!!
            Thread.sleep(200)
        }
        throw AssertionError("Bridge not available after ${timeoutMs}ms")
    }

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        bridge = getBridgeOrWait()
    }

    @Test
    fun cursorBlink_disabled_terminalStaysVisible() {
        bridge!!.setCursorBlinkEnabled(false)
        bridge!!.resetCursorBlink()
        Thread.sleep(2000)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorBlink_enabled_terminalStaysVisibleAfterBlinks() {
        bridge!!.setCursorBlinkEnabled(true)
        bridge!!.setCursorBlinkSpeedMs(200)
        bridge!!.resetCursorBlink()
        Thread.sleep(600)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorNeverRandomlyDisappears() {
        bridge!!.setCursorBlinkEnabled(false)
        bridge!!.resetCursorBlink()
        for (i in 0..9) {
            Thread.sleep(500)
            composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        }
    }

    @Test
    fun cursorShape_switchRoundTrip() {
        bridge!!.setCursorStyle("bar")
        Thread.sleep(100)
        bridge!!.setCursorStyle("underline")
        Thread.sleep(100)
        bridge!!.setCursorStyle("block")
        Thread.sleep(100)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorBlink_speedChange_doesNotCrash() {
        bridge!!.setCursorBlinkEnabled(true)
        bridge!!.setCursorBlinkSpeedMs(100)
        Thread.sleep(50)
        bridge!!.setCursorBlinkSpeedMs(530)
        Thread.sleep(50)
        bridge!!.setCursorBlinkSpeedMs(1000)
        Thread.sleep(50)
        bridge!!.resetCursorBlink()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
