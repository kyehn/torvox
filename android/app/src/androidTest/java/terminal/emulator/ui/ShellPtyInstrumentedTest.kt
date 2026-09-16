package terminal.emulator.ui

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.PollEvent
import terminal.emulator.bridge.pollEventJson

/**
 * 系统 Shell PTY 端到端覆盖（对标 sylirre ShellSessionTest）。
 *
 * 经公共 JNI API 自建隔离会话（initSession/feedPty/查询/destroySession），
 * 驱动真实 `/system/bin/sh`：回显、PATH、尺寸下发。退出上报由会话事件通道覆盖，
 * 不在此关闭会话。
 */
@RunWith(JUnit4::class)
class ShellPtyInstrumentedTest {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 20_000L
    }

    private fun appContext() = InstrumentationRegistry.getInstrumentation().targetContext

    private fun withShellSession(body: (Long) -> Unit) {
        val context = appContext()
        val home = context.filesDir.resolve("shell-test-home").apply { mkdirs() }.absolutePath
        val sessionId =
            NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", 2000)
        assertTrue("原生会话创建失败", sessionId != 0L)
        // 输出泵：PTY→VT 由 pollEvent 驱动且仅泵活跃会话；切为活跃并在轮询中持续泵送。
        NativeBridge.switchSession(sessionId)
        try {
            body(sessionId)
        } finally {
            runCatching { NativeBridge.destroySession(sessionId) }
        }
    }

    private fun pumpAndText(sessionId: Long): String? {
        runCatching { NativeBridge.pollEvent() }
        return NativeBridge.getTerminalText(sessionId)
    }

    private fun shellLines(sessionId: Long, command: String): String {
        NativeBridge.feedPty(sessionId, "$command\n".toByteArray(Charsets.UTF_8))
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText(sessionId)?.contains(command.trim()) == true
            }
        assertNotNull("终端未回显: $command", settled)
        Thread.sleep(800)
        return pumpAndText(sessionId).orEmpty()
    }

    @Test
    fun sessionShowsShellPrompt() {
        withShellSession { sessionId ->
            // 规范覆盖：启动后 shell prompt 正常显示，首行不被吞。
            val promptSeen =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    val text = pumpAndText(sessionId).orEmpty()
                    text.contains("$") || text.contains("#")
                }
            val text = pumpAndText(sessionId).orEmpty()
            assertNotNull("Shell prompt 必须显示, 实际: ${text.takeLast(200)}", promptSeen)
        }
    }

    @Test
    fun shellEchoesCommandOutput() {
        withShellSession { sessionId ->
            val marker = "SHELL_ALIVE_${System.currentTimeMillis() % 100000}"
            val text = shellLines(sessionId, "echo $marker")
            assertTrue("Shell 输出必须包含标记 $marker", text.contains(marker))
        }
    }

    @Test
    fun pathIncludesSystemBin() {
        withShellSession { sessionId ->
            val text = shellLines(sessionId, "echo \$PATH")
            assertTrue("PATH 必须包含 /system/bin, 实际尾部: ${text.takeLast(200)}", text.contains("/system/bin"))
        }
    }

    @Test
    fun resizeDeliveredToShell() {
        withShellSession { sessionId ->
            NativeBridge.resize(sessionId, 30, 100)
            val text = shellLines(sessionId, "stty size")
            assertTrue("尺寸必须下发到 Shell (30 100), 实际尾部: ${text.takeLast(200)}", text.contains("30 100"))
        }
    }

    @Test
    fun workingDirectoryIsReported() {
        withShellSession { sessionId ->
            val text = shellLines(sessionId, "pwd")
            assertTrue("工作目录必须可读, 实际尾部: ${text.takeLast(200)}", text.contains("/"))
        }
    }

    @Test
    fun shellExitReportsWaitpidCode() {
        withShellSession { sessionId ->
            // 退出码必须透出 waitpid 实测值，而非固定值。
            NativeBridge.feedPty(sessionId, "exit 42\n".toByteArray(Charsets.UTF_8))
            val exit = awaitSessionExit(sessionId)
            assertEquals("退出码必须透出 waitpid 实测值", 42, exit?.code)
        }
    }

    @Test
    fun shellKilledBySignalReports128PlusSignal() {
        withShellSession { sessionId ->
            // 信号致死按 128+信号值上报（SIGKILL=9），不得呈现为正常退出码 0。
            NativeBridge.feedPty(sessionId, "kill -9 $$\n".toByteArray(Charsets.UTF_8))
            val exit = awaitSessionExit(sessionId)
            assertEquals("信号致死必须上报 128+信号值 (SIGKILL=9)", 137, exit?.code)
        }
    }

    private fun awaitSessionExit(sessionId: Long): PollEvent.Exit? {
        var exit: PollEvent.Exit? = null
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                val json = runCatching { NativeBridge.pollEvent() }.getOrNull()
                val event = json?.let { runCatching { pollEventJson.decodeFromString<PollEvent>(it) }.getOrNull() }
                if (event is PollEvent.Exit && event.sessionId == sessionId) {
                    exit = event
                    true
                } else {
                    false
                }
            }
        assertNotNull("会话退出事件必须上报: $sessionId", seen)
        return exit
    }
}
