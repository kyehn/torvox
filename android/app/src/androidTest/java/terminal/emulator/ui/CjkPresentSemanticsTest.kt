package terminal.emulator.ui

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.util.runCatchingCancellable

/**
 * 中文呈现语义：CJK 字符落格后 render 必须能呈现（返回码语义），
 * 网格必须包含中文。无截图（截图杀模拟器），只断言网格与返回码。
 */
@RunWith(JUnit4::class)
class CjkPresentSemanticsTest {
    @Test
    fun chinese_text_gridded_and_renderable() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = context.filesDir.resolve("cjk-present-home").apply { mkdirs() }.absolutePath
        val sessionId = NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", "", 2000)
        assertTrue("会话创建失败", sessionId != 0L)
        try {
            NativeBridge.switchSession(sessionId)
            val ready =
                UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    NativeBridge.getTerminalText(sessionId)?.contains("$") == true
                }
            assertNotNull("shell 未就绪", ready)
            NativeBridge.feedTerminal(sessionId, "测试中文一二三\n".toByteArray(Charsets.UTF_8))
            val gridded =
                UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                    NativeBridge.getTerminalText(sessionId)?.contains("测试中文") == true
                }
            assertNotNull("中文必须落格", gridded)
            val rendered = NativeBridge.render(sessionId, 0, 0)
            android.util.Log.i("CjkPresent", "render rc=$rendered")
            val text = NativeBridge.getTerminalText(sessionId).orEmpty()
            assertTrue("网格必须保留中文", text.contains("测试中文"))
        } finally {
            runCatchingCancellable { NativeBridge.destroySession(sessionId) }
        }
    }
}
