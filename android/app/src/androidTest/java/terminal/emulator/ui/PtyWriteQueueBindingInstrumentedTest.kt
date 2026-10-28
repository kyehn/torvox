package terminal.emulator.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.bridge.Bridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.waitForSession

/** 异步 PTY 写入队列必须跟随冷启动与活动会话切换重新绑定。 */
class PtyWriteQueueBindingInstrumentedTest : TerminalLogcatTest() {
    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun surface(): TerminalSurface =
        findTerminalSurface(composeTestRule.activity) as TerminalSurface

    private fun onMain(block: () -> Unit) {
        InstrumentationRegistry.getInstrumentation().runOnMainSync(block)
    }

    @Test
    fun queueFollowsColdStartBridgeAndSessionSwitch() {
        composeTestRule.waitForSession()
        val firstBridge = composeTestRule.awaitBridge()
        val activity = composeTestRule.activity

        val initiallyBound =
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 50) {
                var bound = false
                onMain { bound = surface().hasActivePtyWriteQueue() }
                bound
            }
        assertNotNull("首个 Bridge 就绪后必须安装异步写入队列", initiallyBound)

        var firstSessionId = 0L
        onMain { firstSessionId = activity.runtime.state.value.activeSessionId }
        activity.terminalViewModel.createSession()

        var nextBridge: Bridge? = null
        val switched =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                onMain {
                    val active = activity.runtime.bridge()
                    if (active != null && active !== firstBridge) nextBridge = active
                }
                nextBridge != null
            }
        assertNotNull("新会话必须在 20s 内替换活动 Bridge", switched)
        assertTrue("新会话 ID 必须变化", activity.runtime.state.value.activeSessionId != firstSessionId)

        val rebound =
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 50) {
                var bound = false
                onMain { bound = surface().hasActivePtyWriteQueue() }
                bound
            }
        assertNotNull("切换会话后队列必须重绑到新 Bridge", rebound)
        onMain {
            assertNull("旧 Bridge 不得继续持有已切换视图的写入闭包", firstBridge.ptyWriteQueue)
        }

        val replacementSessionId = activity.runtime.state.value.activeSessionId
        activity.terminalViewModel.closeSession(replacementSessionId)
        val restored =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                activity.runtime.state.value.activeSessionId == firstSessionId
            }
        assertNotNull("用例结束前必须恢复原会话，避免污染共用运行时", restored)
    }
}
