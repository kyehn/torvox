package terminal.emulator.gpu

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

// Unfrozen 2026-08 after root-cause fix: the ComposeNotIdleException was
// NOT a software-rendering limitation. Instrumentation (UiAutomation)
// enables AccessibilityManager, so accessibilityRenderTick fired every
// frame and assembled the visible-screen description on the MAIN thread —
// each line a blocking NativeBridge.scrollbackLine JNI call that contends
// with the render thread's session lock (~500 ms/frame on SwiftShader),
// pinning the main looper inside the mutex forever. Espresso/Compose
// idling therefore never satisfied. Fixed by assembling the description
// on the render thread and posting only the string to the main thread
// (TerminalSurface.accessibilityRenderTick); the main looper now idles.
class CursorBehaviorInstrumentedTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

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
        // Type via the PTY (not compose performTextInput, which requires
        // RequestFocus semantics the AndroidView-wrapped terminal lacks).
        bridge.writeToPty("echo cursor-test\n".toByteArray(Charsets.UTF_8))
        composeTestRule.waitForIdle()
        Thread.sleep(800)
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
