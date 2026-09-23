package terminal.emulator.ui

import android.view.KeyEvent
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * CTRL 粘滞 + c 端到端中断（对标 sylirre TerminalUiTest.stickyCtrlInterruptsCommand）。
 *
 * 真实链路：修饰键栏 CTRL 粘滞置位 → 硬件键 KEYCODE_C 经 onKeyDown →
 * processKeyEvent 折叠 0x03 → PTY 行规程 ISIG → shell 前台进程组 SIGINT →
 * sleep 中断 → echo rc=$? 必须 130（128+SIGINT）。证明折叠出的是真实 ^C。
 */
@RunWith(JUnit4::class)
class StickyCtrlInterruptInstrumentedTest {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 20_000L
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun pumpAndText(): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    @Test
    fun stickyCtrlPlusCInterruptsRunningCommand() {
        composeTestRule.waitForSession()
        // 桥单次读取：会话孵化中为 null，由调用方轮询重试（getBridge 契约）。
        var bridgeReady: terminal.emulator.bridge.Bridge? = null
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            bridgeReady = composeTestRule.getBridge()
            bridgeReady != null
        }
        val bridge =
            bridgeReady ?: throw AssertionError("运行时桥必须就绪（30s 未孵化）")
        // 等 prompt 就绪（shell 空闲后方可启动长命令）。
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                val text = pumpAndText().orEmpty()
                text.contains("$") || text.contains("#")
            }
        assertNotNull("shell prompt 未出现", promptSeen)
        // 启动长命令（真实 shell 进程）。
        bridge.writeToPty("sleep 100\n".toByteArray(Charsets.UTF_8))
        // CTRL 粘滞置位（一次性，下一键消费）。
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.waitForIdle()
        // 硬件键 c 直达 surface onKeyDown：折叠 0x03 → PTY → SIGINT。
        val surface = findTerminalSurface(composeTestRule.activity)
        surface.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_C))
        surface.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_C))
        // sleep 被中断后回到 prompt，查询退出码。
        Thread.sleep(800)
        bridge.writeToPty("echo rc=\$?\n".toByteArray(Charsets.UTF_8))
        val interrupted =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText()?.contains("rc=130") == true
            }
        val text = pumpAndText().orEmpty()
        assertNotNull("CTRL+c 必须产生真实 ^C 中断（rc=130 未出现）, 实际尾部: ${text.takeLast(200)}", interrupted)
        assertTrue("rc=130 必须出现在终端文本中", text.contains("rc=130"))
    }
}
