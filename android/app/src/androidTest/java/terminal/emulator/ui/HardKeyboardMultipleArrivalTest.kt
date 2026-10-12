package terminal.emulator.ui

import android.os.SystemClock
import android.view.KeyEvent
import android.view.inputmethod.EditorInfo
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 硬键盘多字符到达证明（5.3 / 6.3 ACTION_MULTIPLE 接线）：
 * `ACTION_MULTIPLE` 键码为未定义值，走键码映射只会产出空，本仓按 `characters`
 * 码点直提，故含非 ASCII 的标记必须落格，且只落一次（重投即重复文本）。
 *
 * 前置 ETX 清场与回显健康探针：共用会话的行编辑状态跨用例残留，且回显链
 * 不健康时失败会误记到多字符通路。
 */
@RunWith(JUnit4::class)
class HardKeyboardMultipleArrivalTest : TerminalLogcatTest() {
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

    /** 主线程同步投递，返回连接真实的受理结果（异步投递会读到初值）。 */
    @Suppress("DEPRECATION")
    private fun sendMultipleCharacters(marker: String): Boolean {
        var accepted = false
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val connection =
                findTerminalSurface(composeTestRule.activity).onCreateInputConnection(EditorInfo())
            accepted = connection.sendKeyEvent(KeyEvent(SystemClock.uptimeMillis(), marker, 0, 0))
        }
        return accepted
    }

    @Test
    fun multipleEventCharactersArriveExactlyOnce() {
        composeTestRule.waitForSession()
        var bridgeReady: terminal.emulator.bridge.Bridge? = null
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            bridgeReady = composeTestRule.getBridge()
            bridgeReady != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", bridgeReady)
        val bridge = checkNotNull(bridgeReady)
        // 清场：中止共用会话遗留的行编辑内容（空行 ETX 无害）。
        bridge.writeToPty("\u0003".toByteArray(Charsets.UTF_8))
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                val text = pumpAndText().orEmpty()
                text.contains("$") || text.contains("#")
            }
        assertNotNull("shell prompt 未出现", promptSeen)
        // 回显链健康探针：不健康则本用例的落格失败归因错误。
        val healthProbe = "ECHO${System.currentTimeMillis() % 100000}"
        bridge.writeToPty("echo $healthProbe\n".toByteArray(Charsets.UTF_8))
        val echoSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText()?.replace("\n", "")?.contains(healthProbe) == true
            }
        assertNotNull("shell 回显链不健康，本用例结论不可归因到多字符通路", echoSeen)

        val marker = "MULTIα${System.currentTimeMillis() % 100000}"
        val accepted = sendMultipleCharacters(marker)
        assertTrue("ACTION_MULTIPLE 必须被受理（暂停/抑制窗口内拒绝属预期失败）", accepted)
        val arrived =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText()?.replace("\n", "")?.contains(marker) == true
            }
        val text = pumpAndText().orEmpty().replace("\n", "")
        assertTrue("多字符标记必须落格，实际尾部: ${text.takeLast(200)}", arrived != null)
        assertTrue(
            "标记只可出现一次（重投即重复文本）",
            Regex(Regex.escape(marker)).findAll(text).count() == 1,
        )
        bridge.writeToPty("\n".toByteArray(Charsets.UTF_8))
    }
}
