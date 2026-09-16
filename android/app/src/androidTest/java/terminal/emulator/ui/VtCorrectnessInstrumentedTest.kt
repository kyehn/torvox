package terminal.emulator.ui

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge

/**
 * 真机 VT 直达通道的确定性正确性覆盖（对标 sylirre EmulatorVtTest）。
 *
 * 经公共 JNI API 自建隔离会话（initSession/feedTerminal/查询/destroySession），
 * 不依赖共享运行时与 Activity 生命周期；每个用例断言具体行为。
 */
@RunWith(JUnit4::class)
class VtCorrectnessInstrumentedTest {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 15_000L
        private const val ROWS = 24
        private const val COLS = 80
    }

    private fun appContext() = InstrumentationRegistry.getInstrumentation().targetContext

    // 每个用例独占原生会话：行列固定，shell 仅保活（不断言其行为）。
    private fun withSession(body: (Long) -> Unit) {
        val context = appContext()
        val home = context.filesDir.resolve("vt-test-home").apply { mkdirs() }.absolutePath
        val sessionId =
            NativeBridge.initSession(ROWS, COLS, "/system/bin/sh", home, home, "", 2000)
        assertTrue("原生会话创建失败", sessionId != 0L)
        try {
            body(sessionId)
        } finally {
            runCatching { NativeBridge.destroySession(sessionId) }
        }
    }

    private fun feedText(sessionId: Long, text: String) {
        NativeBridge.feedTerminal(sessionId, text.toByteArray(Charsets.UTF_8))
    }

    private fun awaitText(sessionId: Long, needle: String): String {
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                NativeBridge.getTerminalText(sessionId)?.contains(needle) == true
            }
        assertNotNull("VT 文本未渲染: $needle", seen)
        return NativeBridge.getTerminalText(sessionId).orEmpty()
    }

    @Test
    fun plainTextRendersViaVtFeed() {
        withSession { sessionId ->
            val marker = "VT_PLAIN_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, marker)
            val text = awaitText(sessionId, marker)
            assertTrue("纯文本必须可见, 实际尾部: ${text.takeLast(200)}", text.contains(marker))
        }
    }

    @Test
    fun sgrColoredTextRenders() {
        withSession { sessionId ->
            val marker = "VT_SGR_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, "\u001b[31m$marker\u001b[0m")
            val text = awaitText(sessionId, marker)
            assertTrue("SGR 红色文本必须可见: $marker", text.contains(marker))
        }
    }

    @Test
    fun eraseDisplayClearsMarker() {
        withSession { sessionId ->
            val marker = "VT_ERASE_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, marker)
            awaitText(sessionId, marker)
            feedText(sessionId, "\u001b[2J")
            val cleared =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    NativeBridge.getTerminalText(sessionId)?.contains(marker) == false
                }
            assertNotNull("清屏后标记必须消失: $marker", cleared)
        }
    }

    @Test
    fun lineWrapContinuesText() {
        withSession { sessionId ->
            val suffix = "VT_WRAP_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, "W".repeat(120) + suffix)
            awaitText(sessionId, suffix)
        }
    }

    @Test
    fun resizeKeepsContent() {
        withSession { sessionId ->
            val marker = "VT_RESIZE_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, marker)
            awaitText(sessionId, marker)
            NativeBridge.resize(sessionId, 24, 80)
            // 尺寸调整为异步命令：轮询确认内容在重排后仍然存在。
            val kept =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.getTerminalText(sessionId)?.contains(marker) == true
                }
            assertNotNull("尺寸调整后内容必须保留: $marker", kept)
        }
    }

    @Test
    fun altScreenSwitchReturnsToPrimary() {
        withSession { sessionId ->
            val primary = "VT_PRIMARY_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, primary)
            awaitText(sessionId, primary)
            feedText(sessionId, "\u001b[?1049hALT_MARK")
            awaitText(sessionId, "ALT_MARK")
            feedText(sessionId, "\u001b[?1049l")
            awaitText(sessionId, primary)
        }
    }

    @Test
    fun osc8HyperlinkQueryable() {
        withSession { sessionId ->
            val host = "example.com"
            feedText(sessionId, "\u001b]8;;https://$host\u0007LINK_TAP\u001b]8;;\u0007")
            val text = awaitText(sessionId, "LINK_TAP")
            val row = text.lines().indexOfFirst { it.contains("LINK_TAP") }
            assertTrue("必须定位到超链接行", row >= 0)
            val col = text.lines()[row].indexOf("LINK_TAP") + 1
            val uri =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    NativeBridge.hyperlinkAt(sessionId, row, col)?.contains(host) == true
                }
            assertNotNull("超链接必须可查询到 https://$host", uri)
        }
    }

    private fun awaitCurrentDirectory(sessionId: Long, expected: String) {
        // 工作目录收割紧跟 flush：切活跃并持续泵送，确保回调事件被收割。
        NativeBridge.switchSession(sessionId)
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                runCatching { NativeBridge.pollEvent() }
                NativeBridge.getCurrentDirectory(sessionId) == expected
            }
        assertNotNull("工作目录必须可读: $expected, 实际: ${NativeBridge.getCurrentDirectory(sessionId)}", seen)
    }

    @Test
    fun osc7WorkingDirectoryReadable() {
        withSession { sessionId ->
            // TESTING.md 覆盖要求：OSC 7 工作目录读取（上游透出原样 URL）。
            feedText(sessionId, "\u001b]7;file:///data/test-dir\u0007")
            awaitCurrentDirectory(sessionId, "file:///data/test-dir")
        }
    }

    @Test
    fun osc1337CurrentDirReadable() {
        withSession { sessionId ->
            // DESIGN.md 工作目录跟踪：OSC 1337 CurrentDir 提路径。
            feedText(sessionId, "\u001b]1337;CurrentDir=/data/test-dir\u0007")
            awaitCurrentDirectory(sessionId, "/data/test-dir")
        }
    }

    @Test
    fun osc0TitleQueryable() {
        withSession { sessionId ->
            val title = "VT_TITLE_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, "\u001b]0;$title\u0007")
            val seen =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    NativeBridge.getTitle(sessionId) == title
                }
            assertNotNull("标题必须可查询: $title (实际: ${NativeBridge.getTitle(sessionId)})", seen)
        }
    }
}
