package terminal.emulator.ui

import androidx.test.platform.app.InstrumentationRegistry
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
    fun scrollbackHonorsConfiguredLineCount() {
        // 对标 sylirre scrollbackHonorsConfiguredLineCount：回滚深度受建会
        // 参数约束（withSession 固定 2000 行，此处自建小容量会话）。
        // 上游按页粒度修剪（实际值可高于配置几十到一百行），故断言上限
        // 生效（远小于无约束保留量）而非精确等于配置值。
        val context = appContext()
        val home = context.filesDir.resolve("vt-test-home").apply { mkdirs() }.absolutePath
        val cap = 10
        val sessionId = NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", cap)
        assertTrue("原生会话创建失败", sessionId != 0L)
        try {
            val stamp = System.currentTimeMillis() % 100000
            // 上游按页粒度修剪（不满一页不剪）：必须喂足跨页量级才能观测到上限生效。
            val total = 2000
            val payload = (1..total).joinToString("") { "CAP_%04d_$stamp\r\n".format(it) }
            feedText(sessionId, payload)
            val last = "CAP_%04d_$stamp".format(total)
            awaitText(sessionId, last)
            // 上游修剪发生在滚动推进时：追加输出泵一次再判决。
            val extra = "CAP_XTRA_$stamp"
            feedText(sessionId, "$extra\r\n")
            awaitText(sessionId, extra)
            val depth = NativeBridge.scrollbackLength(sessionId)
            assertTrue("回滚上限必须生效（$total 行不得全保留）, 实际深度=$depth", depth < total)
            // 页粒度修剪的完成度因端而异（本机实测 241/2000）：只锁“砍掉一半以上”，
            // 留足页大小方差余量，避免把上游页尺寸波动误判为产品回归。
            assertTrue("修剪必须砍掉一半以上, 实际=$depth", depth < total / 2)
            val full = NativeBridge.getTerminalText(sessionId).orEmpty()
            assertTrue("新行必须保留", full.contains(extra))
            assertTrue(
                "超量旧行必须被淘汰: CAP_0001_$stamp",
                !full.contains("CAP_%04d_$stamp".format(1)),
            )
        } finally {
            runCatching { NativeBridge.destroySession(sessionId) }
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
    fun bellEventIsReportedViaVtFeed() {
        withSession { sessionId ->
            // 对标 sylirre EmulatorVtTest.bellEventIsReported：BEL 直写 VT
            // 解析器（不经 shell），振铃事件必须经事件通道上报。
            // 事件泵仅服务活跃会话：先切活跃再送显（ShellPty 同口径）。
            NativeBridge.switchSession(sessionId)
            feedText(sessionId, "\u0007")
            val seen =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
                    // 事件通道单次消费：一次 poll 即解码，重复 poll 会丢事件。
                    val json = runCatching { NativeBridge.pollEvent() }.getOrNull()
                    val event =
                        json?.let {
                            runCatching { pollEventJson.decodeFromString<PollEvent>(it) }.getOrNull()
                        }
                    event is PollEvent.Bell && event.sessionId == sessionId
                }
            assertNotNull("BEL 振铃事件必须上报: $sessionId", seen)
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
            // 对标 sylirre EmulatorVtTest.arrowKeyEncodingHonorsCursorKeyMode 的模式部分：
            // DECCKM（DEC 私有模式 1）切换必须经 getMode 查询可见。
            feedText(sessionId, "\u001B[?1h")
            val enabled =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    NativeBridge.getMode(sessionId, 1, 0)
                }
            assertNotNull("DECCKM 置位后 getMode(1) 必须为真", enabled)
            feedText(sessionId, "\u001B[?1l")
            val disabled =
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                    !NativeBridge.getMode(sessionId, 1, 0)
                }
            assertNotNull("DECCKM 复位后 getMode(1) 必须为假", disabled)
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
                    false,
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
    fun isCellEmptyDistinguishesContent() {
        withSession { sessionId ->
            // 有可打印码点的格非空，视口底部远端格为空。
            val marker = "CELLEMPTY_${System.currentTimeMillis() % 100000}"
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
            assertTrue("必须定位到标记行: $marker", foundRow >= 0)
            assertTrue(
                "标记格必须非空",
                !NativeBridge.isCellEmpty(sessionId, foundRow, foundCol),
            )
            val bottomRow = depth + ROWS - 1
            assertTrue(
                "视口底部远端格必须为空",
                NativeBridge.isCellEmpty(sessionId, bottomRow, COLS - 1),
            )
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
