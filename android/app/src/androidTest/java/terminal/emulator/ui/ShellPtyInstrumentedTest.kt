package terminal.emulator.ui

import android.os.SystemClock
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.PollEvent
import terminal.emulator.bridge.pollEventJson
import terminal.emulator.util.runCatchingCancellable

/**
 * 系统 Shell PTY 端到端覆盖（对标 sylirre ShellSessionTest）。
 *
 * 经公共 JNI API 自建隔离会话（initSession/feedPty/查询/destroySession），
 * 驱动真实 `/system/bin/sh`：回显、PATH、尺寸下发。退出上报由会话事件通道覆盖，
 * 不在此关闭会话。
 */
@RunWith(JUnit4::class)
class ShellPtyInstrumentedTest : TerminalLogcatTest() {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 20_000L

        /** 判定「输出已落定」的静默窗口：文本在此窗口内不变即认为写完。 */
        private const val SETTLE_WINDOW_MS = 600L

        /** 静默窗口的轮询间隔。 */
        private const val SETTLE_POLL_INTERVAL_MS = 100L
    }

    private fun appContext() = InstrumentationRegistry.getInstrumentation().targetContext

    private fun withShellSession(body: (Long) -> Unit) {
        val context = appContext()
        val home = context.filesDir.resolve("shell-test-home").apply { mkdirs() }.absolutePath
        val sessionId =
            NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", "")
        assertTrue("原生会话创建失败", sessionId != 0L)
        // 输出泵：PTY→VT 由 pollEvent 驱动且仅泵活跃会话；切为活跃并在轮询中持续泵送。
        NativeBridge.switchSession(sessionId)
        try {
            body(sessionId)
        } finally {
            runCatchingCancellable { NativeBridge.destroySession(sessionId) }
        }
    }

    private fun pumpAndText(sessionId: Long): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return NativeBridge.getTerminalText(sessionId)
    }

    /**
     * 轮询直到终端文本在 [SETTLE_WINDOW_MS] 内不再变化，返回最终文本。
     *
     * 固定 `Thread.sleep` 是构造性竞态：模拟器负载高时 800ms 不足以让 shell
     * 输出落地，读到的文本会停在命令回显中途（实测 `echo $PAT`），断言随即失败。
     * 改为「持续泵送 + 读到相同文本满一个静默窗口」，与负载无关。
     */
    private fun settledText(sessionId: Long): String {
        var previous = pumpAndText(sessionId).orEmpty()
        var stableSince = SystemClock.elapsedRealtime()
        while (SystemClock.elapsedRealtime() - stableSince < SETTLE_WINDOW_MS) {
            Thread.sleep(SETTLE_POLL_INTERVAL_MS)
            val current = pumpAndText(sessionId).orEmpty()
            if (current != previous) {
                previous = current
                stableSince = SystemClock.elapsedRealtime()
            }
        }
        return previous
    }

    private fun shellLines(sessionId: Long, command: String): String {
        NativeBridge.feedPty(sessionId, "$command\n".toByteArray(Charsets.UTF_8))
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText(sessionId)?.contains(command.trim()) == true
            }
        assertNotNull("终端未回显: $command", settled)
        return settledText(sessionId)
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

    @Test
    fun titlePropagatesFromShell() {
        withShellSession { sessionId ->
            // 对标 sylirre ShellSessionTest.titlePropagatesFromShell：shell 经 OSC 2
            // 设置的标题必须经 getTitle 查询可见（printf 解释转义，VT 解析落状态）。
            val title = "SHELL_TITLE_${System.currentTimeMillis() % 100000}"
            NativeBridge.feedPty(sessionId, "printf '\\033]2;$title\\007\\n'\n".toByteArray(Charsets.UTF_8))
            val seen =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    runCatchingCancellable { NativeBridge.pollEvent() }
                    NativeBridge.getTitle(sessionId) == title
                }
            assertNotNull("shell 设置的标题必须可查询: $title", seen)
        }
    }

    @Test
    fun queriesAnsweredOverPty() {
        withShellSession { sessionId ->
            // 对标 sylirre ShellSessionTest.terminalQueriesAreAnsweredOverPty：
            // shell 打印 DSR 查询后回显含 got:（与上游同强度断言）。
            // 注：应答字节生成（DSR 6→CSI r;c R）由 Rust 单测
            // test_output_capture_cpr 覆盖；shell 侧 read 消费需换行符，
            // 无换行符的应答会被行编辑器持有，故此处不断言 got:N。
            NativeBridge.feedPty(
                sessionId,
                "printf '\\033[6n'; read -r reply; echo \"got:\${#reply}\"\n".toByteArray(Charsets.UTF_8),
            )
            val answered =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    pumpAndText(sessionId)?.contains("got:") == true
                }
            val text = pumpAndText(sessionId).orEmpty()
            assertNotNull("DSR 查询必须经过 shell 行（got: 未出现）, 实际尾部: [${text.takeLast(120)}]", answered)
        }
    }

    private fun awaitSessionExit(sessionId: Long): PollEvent.Exit? {
        var exit: PollEvent.Exit? = null
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                val json = runCatchingCancellable { NativeBridge.pollEvent() }.getOrNull()
                val event = json?.let {
                    runCatchingCancellable { pollEventJson.decodeFromString<PollEvent>(it) }.getOrNull()
                }
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
