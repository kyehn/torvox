package terminal.emulator.ui

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.Context
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
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
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

/**
 * End-to-end OSC 52 clipboard write with a real assertion.
 *
 * Feeds `ESC ] 52 ; c ; <base64> BEL` through the REAL terminal input
 * ([writeToPty][terminal.emulator.bridge.Bridge.writeToPty]) and asserts the Android system
 * clipboard contains the decoded marker — Ghostty ClipboardEvent → JNI → ClipboardManager, no
 * screenshots, no OCR.
 */
@RunWith(JUnit4::class)
class Osc52ClipboardInstrumentedTest {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 15_000L
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    // setPrimaryClip：API 36 起废弃但无替代，仍是唯一客户端 API。
    @SuppressLint("DeprecatedCall")
    fun osc52_sequence_sets_system_clipboard() {
        composeTestRule.waitForSession()
        // 会话孵化慢于 UI 呈现：TerminalScreen 可见时活动会话可能仍为 0
        //（runtime.bridge() 取活动会话桥），单次直读必竞态。与 ImePopup 同门控轮询。
        val bridgeReady =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                composeTestRule.getBridge() != null
            }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", bridgeReady)
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        val activity = composeTestRule.activity
        val clipboard =
            activity.getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
        // Clear first so the assertion cannot pass on stale content.
        activity.runOnUiThread { clipboard.setPrimaryClip(ClipData.newPlainText("test", "")) }

        val marker = "OSC52_ALIVE_${System.currentTimeMillis() % 100000}"
        val encoded =
            android.util.Base64.encodeToString(
                marker.toByteArray(Charsets.UTF_8),
                android.util.Base64.NO_WRAP,
            )
        // 经 shell 打印序列：程序经 stdout 输出才是 OSC52 被 Ghostty 解析的
        // 真实路径（vim/tmux 皆如此）。裸写 stdin 依赖行规程回显，ECHOCTL 下
        // ESC 被回显为 ^[，序列永不到达解析器——等 prompt 确认 shell 就绪后执行。
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                runCatching { NativeBridge.pollEvent() }
                bridge.getTerminalText().orEmpty().contains("$") ||
                    bridge.getTerminalText().orEmpty().contains("#")
            }
        assertNotNull("shell prompt 未出现", promptSeen)
        assertTrue(
            "PTY write rejected",
            bridge.writeToPty("printf '\u001B]52;c;$encoded\u0007'\n".toByteArray(Charsets.UTF_8)),
        )
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                clipboard.primaryClip?.getItemAt(0)?.text?.toString() == marker
            }
        val actual = clipboard.primaryClip?.getItemAt(0)?.text?.toString()
        assertNotNull("clipboard never received OSC52 marker $marker (got: $actual)", seen)
        assertTrue(
            "clipboard must equal marker, got: $actual",
            actual == marker,
        )
    }
}
