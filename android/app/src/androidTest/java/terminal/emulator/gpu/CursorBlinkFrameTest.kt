
package terminal.emulator.gpu

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTextInput
import org.junit.Assert
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

@org.junit.Ignore("All four tests call Bridge.setCursorBlink*/resetCursorBlink/setCursorStyle which are log-only implemented (native query path is wired) — nothing reaches JNI, so the tests have zero assertion value ")
class CursorBlinkFrameTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun bridge_cursorBlinkMethods_areReachable() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

        bridge.setCursorBlinkEnabled(true)
        bridge.setCursorBlinkSpeedMs(300)
        bridge.resetCursorBlink()

        // If no UnsatisfiedLinkError thrown, native methods are loaded
    }

    @Test
    fun bridge_cursorBlinkEnabled_toggleDoesNotThrow() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

        bridge.setCursorBlinkEnabled(false)
        bridge.setCursorBlinkEnabled(true)
        bridge.setCursorBlinkSpeedMs(750)
        bridge.resetCursorBlink()
    }

    @Test
    fun bridge_cursorStyleMethods_areReachable() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

        bridge.setCursorStyle("block")
        bridge.setCursorStyle("underline")
        bridge.setCursorStyle("bar")
    }

    @Test
    fun bridge_multipleSpeedSettings_applySequentially() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

        bridge.setCursorBlinkEnabled(true)
        bridge.setCursorBlinkSpeedMs(100)
        bridge.setCursorBlinkSpeedMs(530)
        bridge.setCursorBlinkSpeedMs(1000)
        bridge.resetCursorBlink()
    }
}
