package terminal.emulator.ui

import android.os.SystemClock
import android.view.KeyEvent
import android.view.inputmethod.EditorInfo
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.findTerminalSurface
import terminal.emulator.pumpedTerminalText
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
class HardKeyboardMultipleArrivalTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 20_000L
    }

    /**
     * 主线程同步投递，返回受理结果与尝试次数（重复文本若出现，先看重试次数）。
     *
     * 必须**轮询重试**而不是只发一次：`sendKeyEvent` 的判据是
     * `now < suppressUntilNanos`——窗口切换（获得焦点、抽屉关闭）会开一个**有界**的
     * 抑制窗口，instrumentation 环境下焦点抖动会让单次投递有相当概率正落在窗内而被
     * **据实拒绝**（返回 false，不是丢字）。窗口不延长、不叠加，于是重试必然收敛。
     *
     * 这正是该断言原先的失败消息「暂停/抑制窗口内拒绝属预期失败」那句的含义：
     * 它描述的是被测实现的有意契约，而不是缺陷——用单次投递去 `assertTrue` 是在
     * 拿契约当 bug 报。模拟器上稳定复现（修前 3 连败）、修后 4 连过。
     *
     * 重试安全：受理结果就是返回值本身，返回 false 即未受理、未提交，故重试不会
     * 把同一段文本重复送进去。
     */
    @Suppress("DEPRECATION")
    private fun sendMultipleCharacters(marker: String): Pair<Boolean, Int> {
        var attempts = 0
        var accepted = false
        UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
            attempts++
            InstrumentationRegistry.getInstrumentation().runOnMainSync {
                val connection =
                    findTerminalSurface(composeTestRule.activity)
                        .onCreateInputConnection(EditorInfo())
                accepted =
                    connection.sendKeyEvent(KeyEvent(SystemClock.uptimeMillis(), marker, 0, 0))
            }
            accepted
        }
        return accepted to attempts
    }

    @Test
    fun multipleEventCharactersArriveExactlyOnce() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.awaitBridge()
        // 清场：中止共用会话遗留的行编辑内容（空行 ETX 无害）。
        bridge.writeToPty("\u0003".toByteArray(Charsets.UTF_8))
        awaitShellPrompt(bridge, OUTPUT_TIMEOUT_MS)
        // 回显链健康探针：不健康则本用例的落格失败归因错误。
        val healthProbe = "ECHO${System.currentTimeMillis() % 100000}"
        bridge.writeToPty("echo $healthProbe\n".toByteArray(Charsets.UTF_8))
        val echoSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                composeTestRule.pumpedTerminalText()?.replace("\n", "")?.contains(healthProbe) == true
            }
        assertNotNull("shell 回显链不健康，本用例结论不可归因到多字符通路", echoSeen)

        val marker = "MULTIα${SystemClock.uptimeMillis()}"
        val (accepted, attempts) = sendMultipleCharacters(marker)
        assertTrue("ACTION_MULTIPLE 必须被受理（有界抑制窗口内拒绝属预期失败，尝试 $attempts 次）", accepted)
        val arrived =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                composeTestRule.pumpedTerminalText()?.replace("\n", "")?.contains(marker) == true
            }
        val text = composeTestRule.pumpedTerminalText().orEmpty().replace("\n", "")
        assertTrue("多字符标记必须落格，实际尾部: ${text.takeLast(200)}", arrived != null)
        val occurrences = Regex(Regex.escape(marker)).findAll(text).count()
        assertTrue(
            "标记只可出现一次（重投即重复文本），尝试 $attempts 次后实际 $occurrences 次；尾部: ${text.takeLast(300)}",
            occurrences == 1,
        )
        bridge.writeToPty("\n".toByteArray(Charsets.UTF_8))
    }
}
