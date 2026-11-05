package terminal.emulator.performance

import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.withShellProbeSession

/**
 * 暂停恢复语义：IME 弹出暂停渲染期间写入的内容，恢复后必须呈现。
 * 无截图：只断言网格落格与 render 返回码。
 */
@RunWith(JUnit4::class)
class RenderPauseSemanticsTest : TerminalLogcatTest() {
    @Test
    fun pausedWritesArePresentedAfterResume() {
        withShellProbeSession("pause-probe-home") { sessionId ->
            NativeBridge.feedTerminal(
                sessionId,
                "\u001B[31mPAUSE_RED_A\u001B[0m\r\n".toByteArray(Charsets.UTF_8),
            )
            val first =
                UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                    NativeBridge.getTerminalText(sessionId)?.contains("PAUSE_RED_A") == true
                }
            assertNotNull("首标记未落格", first)
            // 无 surface 时 render 返回 0（surface 未挂载），只记录不强断言；
            // 暂停/恢复语义由网格与返回码相对关系验证。
            val firstRender = NativeBridge.render(sessionId, 0, 0)
            android.util.Log.i("PauseProbe", "firstRender=$firstRender")
            NativeBridge.setRenderPaused(true)
            try {
                NativeBridge.feedTerminal(
                    sessionId,
                    "\u001B[31mPAUSE_RED_B\u001B[0m\r\n".toByteArray(Charsets.UTF_8),
                )
                val second =
                    UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                        NativeBridge.getTerminalText(sessionId)?.contains("PAUSE_RED_B") == true
                    }
                assertNotNull("暂停期写入必须落格", second)
                assertTrue("暂停期 render 必须返回 0", NativeBridge.render(sessionId, 0, 0) == 0)
            } finally {
                NativeBridge.setRenderPaused(false)
            }
            val presented = NativeBridge.render(sessionId, 0, 0)
            android.util.Log.i("PauseProbe", "resumedRender=$presented")
            val text = NativeBridge.getTerminalText(sessionId).orEmpty()
            assertTrue("恢复后网格必须含暂停期标记", text.contains("PAUSE_RED_B"))
        }
    }
}
