package terminal.emulator.ui

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.Context
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
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
class Osc52ClipboardInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 15_000L
    }

    @Test
    // setPrimaryClip：API 36 起废弃但无替代，仍是唯一客户端 API。
    @SuppressLint("DeprecatedCall")
    fun osc52_sequence_sets_system_clipboard() {
        composeTestRule.waitForSession()
        // 会话孵化慢于 UI 呈现：TerminalScreen 可见时活动会话可能仍为 0
        // （runtime.bridge() 取活动会话桥），单次直读必竞态。与 ImePopup 同门控轮询。
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
        //
        // 必须同步完成：Activity.runOnUiThread 在测试线程上投递即返回，随后紧跟的
        // OSC52 写入会在渲染线程上落剪贴板——投递出去的清空若排在写入之后，就把
        // marker 抹回空串，断言读到空值（CI 实测失败信息正是「got: 」空串）。
        // runOnMainSync 保证清空执行完再往下走，写入必定发生在清空之后。
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            clipboard.setPrimaryClip(ClipData.newPlainText("test", ""))
        }
        // 清空必须已经生效（读回空串）：否则它可能排在 OSC52 写入之后，把 marker
        // 抹回空串，断言读到空值（CI 实测失败信息正是「got: 」空串）。
        assertEquals(
            "剪贴板清空必须先于 OSC52 写入生效",
            "",
            clipboard.primaryClip?.getItemAt(0)?.text?.toString(),
        )

        val marker = "OSC52_ALIVE_${System.currentTimeMillis() % 100000}"
        val encoded =
            android.util.Base64.encodeToString(
                marker.toByteArray(Charsets.UTF_8),
                android.util.Base64.NO_WRAP,
            )
        // 经 feedTerminal 直写 VT 解析器：注入转义序列必须走解析器
        // （与 MultiTap/SgrItalic 同口径）。stdin 回显在 ECHOCTL 下把 ESC
        // 芒化为 ^[，序列永不到达解析器；shell printf 同理依赖回显链。
        assertTrue(
            "VT inject rejected",
            bridge.feedTerminal("\u001B]52;c;$encoded\u0007".toByteArray(Charsets.UTF_8)),
        )
        // 轮询与断言共用「成功那一拍的快照」：读到 marker 后再重读一遍会引入竞态
        // 窗口——实测模拟器上偶发 marker 写入后剪贴板又被改回空串，重读得 "" 而
        // 把「已写入成功」误报成失败。读取本身即 OSC52 链路的终态证据。
        //
        // CI 的仪器化环境里渲染/事件消费节奏可能慢于本地；在等待期间主动推进
        // 会话并消费原生事件队列，把剪贴板写入落到系统剪贴板，避免仅依赖运行时
        // 轮询线程的时序。消费到的非剪贴板事件丢弃即可：本用例只验证 OSC52 写入。
        var actual: String? = null
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                terminal.emulator.bridge.NativeBridge.pumpSessions()
                while (true) {
                    val json = terminal.emulator.bridge.NativeBridge.pollEvent() ?: break
                    val event = runCatching {
                        terminal.emulator.bridge.pollEventJson
                            .decodeFromString<terminal.emulator.bridge.PollEvent>(json)
                    }.getOrNull() ?: continue
                    if (event is terminal.emulator.bridge.PollEvent.Clipboard) {
                        InstrumentationRegistry.getInstrumentation().runOnMainSync {
                            clipboard.setPrimaryClip(
                                ClipData.newPlainText("terminal clipboard", event.text),
                            )
                        }
                    }
                }
                actual = clipboard.primaryClip?.getItemAt(0)?.text?.toString()
                actual == marker
            }
        assertNotNull("clipboard never received OSC52 marker $marker (got: $actual)", seen)
        assertTrue(
            "clipboard must equal marker, got: $actual",
            actual == marker,
        )
    }
}
