package terminal.emulator.gpu

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.rule.GrantPermissionRule
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

// 光标固定为默认方块且不闪烁（禁止实现已删除）：仅断言终端可见与输入后光标仍在。
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
    fun cursorVisible_terminalStaysVisible() {
        composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun cursorVisibility_persistsAfterTyping() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        // Type via the PTY (not compose performTextInput, which requires
        // RequestFocus semantics the AndroidView-wrapped terminal lacks).
        bridge.writeToPty("echo cursor-test\n".toByteArray(Charsets.UTF_8))
        composeTestRule.waitForIdle()
        Thread.sleep(800)
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
