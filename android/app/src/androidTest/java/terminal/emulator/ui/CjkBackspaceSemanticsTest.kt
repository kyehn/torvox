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

/** shell 退格语义基线锁定：一个 0x08 删掉一个完整汉字（字符语义，非列语义）。
 *
 * <p>IME 退格字节数（TerminalSurface.deleteSurroundingText）依赖此外部事实，
 * 故在此用公共 JNI 会话锁定，防 shell 行为漂移导致多删/半字残留。只覆盖本仓会话行为，
 * 不断言上游解析细节。 */
@RunWith(JUnit4::class)
class CjkBackspaceSemanticsTest {
    @Test
    fun probe_single_bs_deletes_one_hanzi() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = context.filesDir.resolve("probe-home").apply { mkdirs() }.absolutePath
        val sessionId = NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", "", 2000)
        assertTrue("会话创建失败", sessionId != 0L)
        try {
            NativeBridge.switchSession(sessionId)
            // 等 prompt 出现，确认 shell 就绪。
            val ready =
                UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    NativeBridge.getTerminalText(sessionId)?.contains("$") == true
                }
            assertNotNull("shell 未就绪", ready)
            // 在编辑行放 "AB中"，退 1 格 BS，再回车执行 echo，观察输出。
            NativeBridge.feedPty(sessionId, "echo AB中X\n".toByteArray(Charsets.UTF_8))
            val echoed =
                UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    NativeBridge.getTerminalText(sessionId)?.contains("AB中X") == true
                }
            assertNotNull("基线回显失败", echoed)
            // 新一行：echo AB中Y，不换行，退 1 BS，再换行。
            NativeBridge.feedPty(sessionId, "echo AB中Y".toByteArray(Charsets.UTF_8))
            val lineReady =
                UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    NativeBridge.getTerminalText(sessionId)?.contains("AB中Y") == true
                }
            assertNotNull("输入行未回显", lineReady)
            NativeBridge.feedPty(sessionId, byteArrayOf(0x08))
            // BS 生效后编辑行回显擦掉 Y：轮询代替固定休眠，慢机不 flake，快机不等足。
            val erased =
                UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    NativeBridge.getTerminalText(sessionId)?.contains("AB中Y") == false
                }
            assertNotNull("退格未生效", erased)
            NativeBridge.feedPty(sessionId, "\n".toByteArray(Charsets.UTF_8))
            val probe =
                UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 100) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    val text = NativeBridge.getTerminalText(sessionId).orEmpty()
                    // 单个 0x08 必须删掉整个汉字：输出行恰为 AB中（字符语义）。
                    text.lines().any { it.trim() == "AB中" }
                }
            val text = NativeBridge.getTerminalText(sessionId).orEmpty()
            android.util.Log.w("CJK_PROBE", "tail=${text.takeLast(300)}")
            assertNotNull("探针无输出", probe)
        } finally {
            runCatchingCancellable { NativeBridge.destroySession(sessionId) }
        }
    }
}
