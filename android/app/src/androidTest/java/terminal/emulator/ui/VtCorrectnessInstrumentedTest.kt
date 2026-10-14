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
 * 真机 VT 直达通道的确定性正确性覆盖（对标 sylirre EmulatorVtTest）。
 *
 * 经公共 JNI API 自建隔离会话（initSession/feedTerminal/查询/destroySession），
 * 不依赖共享运行时与 Activity 生命周期；每个用例断言具体行为。
 */
@RunWith(JUnit4::class)
class VtCorrectnessInstrumentedTest : TerminalLogcatTest() {
    companion object {
        private const val OUTPUT_TIMEOUT_MS = 15_000L
        private const val SETTLE_INTERVAL_MS = 200L
        private const val ROWS = 24
        private const val COLS = 80
    }

    private fun appContext() = InstrumentationRegistry.getInstrumentation().targetContext

    // 每个用例独占原生会话：行列固定，shell 仅保活（不断言其行为）。
    private fun withSession(body: (Long) -> Unit) {
        val context = appContext()
        val home = context.filesDir.resolve("vt-test-home").apply { mkdirs() }.absolutePath
        val sessionId =
            NativeBridge.initSession(ROWS, COLS, "/system/bin/sh", home, home, "", "")
        assertTrue("原生会话创建失败", sessionId != 0L)
        try {
            awaitShellQuiet(sessionId)
            resetGrid(sessionId)
            body(sessionId)
        } finally {
            runCatchingCancellable { NativeBridge.destroySession(sessionId) }
        }
    }

    /**
     * 等 shell 停止输出。提示符经 PTY 异步落到网格，若与用例注入的字节交叠，
     * 光标断言就会读到 shell 的落点而非注入后的位置（实测光标被推到第 50 余列）。
     * 连续两次采样文本不变即视为安静，不依赖提示符的具体形状。
     */
    private fun awaitShellQuiet(sessionId: Long) {
        val deadline = SystemClock.uptimeMillis() + OUTPUT_TIMEOUT_MS
        var previous = NativeBridge.getTerminalText(sessionId)
        while (SystemClock.uptimeMillis() < deadline) {
            Thread.sleep(SETTLE_INTERVAL_MS)
            val current = NativeBridge.getTerminalText(sessionId)
            if (current == previous) return
            previous = current
        }
    }

    /** 清屏并把光标归零，使后续断言只反映本用例注入的字节。 */
    private fun resetGrid(sessionId: Long) {
        feedText(sessionId, "\u001B[2J\u001B[H")
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                NativeBridge.getCursorViewportPacked(sessionId) == 0L
            }
        assertNotNull("清屏后光标必须归零", seen)
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
    fun newlineAndCarriageReturnSemantics() {
        withSession { sessionId ->
            // 对标 sylirre EmulatorVtTest.newlineAndCarriageReturn：
            // \r\n 必须换行且回车，每标记独占一行行首。
            val stamp = System.currentTimeMillis() % 100000
            val first = "NLCR_A_$stamp"
            val second = "NLCR_B_$stamp"
            val third = "NLCR_C_$stamp"
            feedText(sessionId, "$first\r\n$second\r\n$third")
            val text = awaitText(sessionId, third)
            val lines = text.lines()
            for (marker in listOf(first, second, third)) {
                val row = lines.indexOfFirst { it.contains(marker) }
                assertTrue("必须定位到标记行: $marker", row >= 0)
                assertTrue(
                    "\\r\\n 后标记必须独占行首, 实际: [${lines[row]}]",
                    lines[row].startsWith(marker),
                )
            }
        }
    }

    @Test
    fun scrollbackKeepsOverflowInOrder() {
        withSession { sessionId ->
            // 对标 sylirre scrollbackAndViewport + TESTING.md 回滚要求：
            // 超屏旧行按序进入滚区（scrollbackLength 增长），新行在底部。
            val stamp = System.currentTimeMillis() % 100000
            val total = 60 // 远超 24 行视口，必溢出。
            val payload = (1..total).joinToString("") { "SB_%03d_$stamp\r\n".format(it) }
            feedText(sessionId, payload)
            val last = "SB_%03d_$stamp".format(total)
            awaitText(sessionId, last)
            val grown =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    NativeBridge.scrollbackLength(sessionId) > 0
                }
            val depth = NativeBridge.scrollbackLength(sessionId)
            assertNotNull("超屏输出必须进入回滚区, 实际深度=$depth", grown)
            val full = NativeBridge.getTerminalText(sessionId).orEmpty()
            val first = "SB_%03d_$stamp".format(1)
            assertTrue(
                "旧行必须按序在新行之前 (首尾均可见: $depth)",
                full.contains(first) && full.indexOf(first) < full.indexOf(last),
            )
            val visibleTail = full.lines().takeLast(3).joinToString("\n")
            assertTrue("新行必须显示在底部, 实际尾部: [$visibleTail]", visibleTail.contains(last))
        }
    }

    @Test
    fun wideCharacterOccupiesTwoCells() {
        withSession { sessionId ->
            // 对标 sylirre EmulatorVtTest.wideCharacterOccupiesTwoCells +
            // TESTING.md 简体中文显示宽度：CJK 宽字符必须占两列。
            // A(1) + 中(2) + B(1) = 光标落在第 4 列；若按单列处理则为第 3 列。
            feedText(sessionId, "A中B")
            val text = awaitText(sessionId, "中")
            assertTrue("宽字符必须可见", text.contains("中"))
            awaitCursor(sessionId, 0, 4, "宽字符后")
        }
    }

    @Test
    fun cursorMovementSemantics() {
        withSession { sessionId ->
            // 对标 sylirre EmulatorVtTest.cursorMovement：纯文本推进、CUP
            // 绝对定位、CUB/CUF 相对移动，光标坐标必须跟随。
            // 视口行列 0 起（getCursorViewportPacked 高 32 位行、低 32 位列）。
            feedText(sessionId, "AB")
            awaitCursor(sessionId, 0, 2, "纯文本后")
            feedText(sessionId, "\u001B[5;10H")
            awaitCursor(sessionId, 4, 9, "CUP 5;10后")
            feedText(sessionId, "\u001B[3D")
            awaitCursor(sessionId, 4, 6, "左移3后")
            feedText(sessionId, "\u001B[2C")
            awaitCursor(sessionId, 4, 8, "右移2后")
        }
    }

    @Test
    fun cursorKeyModeSwitchesViaDecPrivateMode() {
        withSession { sessionId ->
            // 对标 sylirre EmulatorVtTest.arrowKeyEncodingHonorsCursorKeyMode 的模式部分。
            //
            // 判据取**生产编码路径**而非模式标志查询：DECCKM（DEC 私有模式 1）是否真的
            // 到达编码器读的那份终端状态，唯一可观测的后果是方向键的形式在 CSI 与 SS3
            // 应用光标之间切换。原生 `getMode` 导出已随之删除——它没有生产调用方，
            // 而留下的判据比模式标志更强。
            fun encodedArrowKey(): String = NativeBridge.encodeKeyEvent(
                sessionId,
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                0,
                0,
                0,
                android.view.KeyEvent.ACTION_DOWN,
            ).toString(Charsets.US_ASCII)

            assertEquals("默认状态下方向键走 CSI 形式", "\u001b[A", encodedArrowKey())
            feedText(sessionId, "\u001B[?1h")
            val enabled =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    encodedArrowKey() == "\u001bOA"
                }
            assertNotNull("DECCKM 置位后方向键须换成 SS3 应用光标形式", enabled)
            feedText(sessionId, "\u001B[?1l")
            val disabled =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    encodedArrowKey() == "\u001b[A"
                }
            assertNotNull("DECCKM 复位后方向键须回到 CSI 形式", disabled)
        }
    }

    @Test
    fun selectionTextExtractsFedWord() {
        withSession { sessionId ->
            // 对标 sylirre EmulatorVtTest.selectWordHighlightsAndExtractsText 的文本部分
            // （反白属性因架构分叉不搬）：直写标记行，经 scrollbackLine 定位绝对行，
            // selectionText 按网格坐标提取必须原样返回。
            val marker = "SEL_WD_${System.currentTimeMillis() % 100000}"
            feedText(sessionId, marker)
            awaitText(sessionId, marker)
            val depth = NativeBridge.scrollbackLength(sessionId)
            var foundRow = -1
            var foundCol = -1
            for (row in 0..(depth + ROWS)) {
                val line = NativeBridge.scrollbackLine(sessionId, row) ?: continue
                val col = line.indexOf(marker)
                if (col >= 0) {
                    foundRow = row
                    foundCol = col
                    break
                }
            }
            assertTrue("必须经 scrollbackLine 定位到标记行: $marker", foundRow >= 0)
            val extracted =
                NativeBridge.selectionText(
                    sessionId,
                    foundRow,
                    foundCol,
                    foundRow,
                    foundCol + marker.length,
                )
            assertTrue("选区提取必须原样返回标记, 实际: [$extracted]", extracted == marker)
        }
    }

    private fun awaitCursor(sessionId: Long, row: Int, col: Int, what: String) {
        // 光标查询走 VT 状态（异步通道），轮询而非单次读取。
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                val packed = NativeBridge.getCursorViewportPacked(sessionId)
                packed >= 0 &&
                    (packed shr 32).toInt() == row &&
                    (packed and 0xffffffffL).toInt() == col
            }
        val packed = NativeBridge.getCursorViewportPacked(sessionId)
        assertNotNull("光标必须到达($row,$col) [$what], 实际packed=$packed", seen)
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
    fun altScreenStateMirrorsSwitch() {
        withSession { sessionId ->
            // 备用屏状态查询必须与切换语义一致：主屏 false，切入 true，切回 false。
            assertTrue("初始必须在主屏", !NativeBridge.getAltScreenState(sessionId))
            feedText(sessionId, "\u001b[?1049h")
            val entered =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    NativeBridge.getAltScreenState(sessionId)
                }
            assertNotNull("切入备用屏后状态必须为真", entered)
            feedText(sessionId, "\u001b[?1049l")
            val exited =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    !NativeBridge.getAltScreenState(sessionId)
                }
            assertNotNull("切回主屏后状态必须为假", exited)
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
