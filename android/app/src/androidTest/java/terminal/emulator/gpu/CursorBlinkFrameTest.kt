
package terminal.emulator.gpu

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTextInput
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

// Frozen via @Ignore until 2026-08: original blocker was "log-only bridge"
// (now fully wired to JNI via NativeBridge) plus Compose-idling timeouts on
// software-rendered emulators. Re-enabled and verified on the KVM-accelerated
// emulator (4/4 green; one flaky ANR observed on a cold re-run, retry works).
// See docs/standards/TESTING.md §Instrumented 冻结与质量审计.
class CursorBlinkFrameTest {
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
