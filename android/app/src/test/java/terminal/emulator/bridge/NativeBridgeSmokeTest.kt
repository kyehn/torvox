package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.settings.SettingsRepository
import java.io.File
import kotlin.system.measureTimeMillis

/**
 * JVM-side JNI round-trip tests — no emulator, no Android device.
 *
 * Loads the HOST-built `libnative.so` (same Rust code as the Android target, built with
 * `nix develop --command cargo build --manifest-path native/Cargo.toml --profile release`) and drives the real JNI
 * bridge: initSession → feedTerminal → getTitle/getTerminalText → destroySession.
 *
 * ## Why this exists (what pure-Rust tests cannot cover)
 *
 * Rust 侧测试覆盖 VT 解析、单元格管线和事件队列（直接调 Rust）。它
 * cannot exercise the JNI boundary layer:
 *
 * - JString→String / String→JString conversion (UTF-16 round-trip, NUL, non-ASCII) — only a real
 *   JVM produces/reads JNI strings
 * - jbyteArray→Vec<u8> input conversion ([feedTerminal] is binary-safe)
 * - env.throw_new exception paths (IllegalArgumentException on bad args, IllegalStateException on a missing session)
 * - the full Kotlin→JNI→command channel→VT thread→ghostty parser→query round-trip; `vt_write` hands
 *   the buffer to the VT thread via `try_send`, so a broken command channel or dead VT thread only
 *   shows here (previously only the emulator caught it — heavy and black-box)
 *
 * Graphics exports (attachWindow/render/captureFrame) need ANativeWindow and stay on the emulator;
 * everything exercised here is pure CPU logic.
 *
 * Locating the library: unit tests run with cwd = `android/app/`, so the path is the fixed
 * `../../native/target/release/libnative.so`. No candidate probing and no environment override — the
 * library must already be there (built as above, the step the `check` workflow runs before
 * `check-gradle.nu`), or the load fails loudly.
 */
class NativeBridgeSmokeTest {
    private companion object {
        /** 固定路径：缺失时 System.load 自身抛错，不再探测候选位置。 */
        private const val HOST_LIBRARY_PATH = "../../native/target/release/libnative.so"

        /** Real shell on the dev host (CI runner). Android uses /system/bin/sh. */
        private const val HOST_SHELL = "/bin/sh"

        private const val POLL_TIMEOUT_MS = 5_000L
        private const val POLL_INTERVAL_MS = 25L
    }

    @Before
    fun loadNativeLibrary() {
        System.load(File(HOST_LIBRARY_PATH).absolutePath)
    }

    /** Poll `probe` until it returns true or [POLL_TIMEOUT_MS] elapses. */
    private fun awaitTrue(what: String, probe: () -> Boolean): Boolean {
        val deadline = System.currentTimeMillis() + POLL_TIMEOUT_MS
        while (System.currentTimeMillis() < deadline) {
            if (probe()) return true
            Thread.sleep(POLL_INTERVAL_MS)
        }
        println("awaitTrue timeout waiting for: $what")
        return probe()
    }

    /** Spawn a real session, run `block`, and always clean up afterwards. */
    private fun withSession(block: (Long) -> Unit) {
        val sessionId =
            NativeBridge.initSession(
                rows = 24,
                cols = 80,
                shell = HOST_SHELL,
                home = System.getenv("HOME") ?: "",
                workingDirectory = System.getProperty("user.dir") ?: "",
                prefix = "",
                mkshrcPath = "",
                scrollbackLines = 2000,
            )
        assertTrue("initSession must return a positive session id, got $sessionId", sessionId > 0)
        try {
            block(sessionId)
        } finally {
            val destroyed = NativeBridge.destroySession(sessionId)
            assertTrue("destroySession must report success", destroyed)
        }
    }

    @Test
    fun `initSession spawns a session and destroySession removes it`() {
        val before = NativeBridge.listSessions().size
        withSession { sessionId ->
            assertTrue(
                "registry must now contain the session",
                NativeBridge.listSessions().contains(sessionId),
            )
        }
        assertEquals("session must be removed again", before, NativeBridge.listSessions().size)
    }

    @Test
    fun `feedTerminal OSC 0 title round-trips through the live VT thread`() {
        withSession { sessionId ->
            // OSC 0 sets the window/title; vt_write is async (try_send to the
            // VT thread), so poll getTitle until the parser applied it.
            NativeBridge.feedTerminal(sessionId, "\u001b]0;JNI-SMOKE\u001b\\".toByteArray())
            val applied =
                awaitTrue("title applied") {
                    NativeBridge.getTitle(sessionId) == "JNI-SMOKE"
                }
            assertTrue("OSC 0 title must be applied by the VT thread", applied)
        }
    }

    @Test
    fun `getSessionTitles returns titles in the requested order`() {
        withSession { firstId ->
            withSession { secondId ->
                NativeBridge.feedTerminal(firstId, "\u001b]0;FIRST\u001b\\".toByteArray())
                NativeBridge.feedTerminal(secondId, "\u001b]0;SECOND\u001b\\".toByteArray())
                assertTrue(
                    "两个标题都必须被 VT 线程应用",
                    awaitTrue("titles applied") {
                        NativeBridge.getTitle(firstId) == "FIRST" &&
                            NativeBridge.getTitle(secondId) == "SECOND"
                    },
                )
                // 顺序由实参决定而非注册表顺序：调用方按自己的会话表回填，
                // 对不上就会把标题写到别的会话上。
                val forward = NativeBridge.getSessionTitles(longArrayOf(firstId, secondId))
                assertEquals(listOf("FIRST", "SECOND"), forward?.toList())
                val reversed = NativeBridge.getSessionTitles(longArrayOf(secondId, firstId))
                assertEquals(listOf("SECOND", "FIRST"), reversed?.toList())
            }
        }
    }

    @Test
    fun `getSessionTitles reports unknown sessions as null without throwing`() {
        withSession { sessionId ->
            // 活着的会话没有标题时是**空串**，未知会话才是 null：调用方据此区分
            // 「这个会话还没有标题」与「会话没了」，后者要能触发列表重建。
            val titles = NativeBridge.getSessionTitles(longArrayOf(sessionId, 999_999L))
            assertEquals(
                "未知会话须为 null，无标题会话须为空串",
                listOf("", null),
                titles?.toList(),
            )
        }
        // 空入参不得抛：会话列表为空时每轮刷新都会走到它。
        assertEquals(emptyList<String?>(), NativeBridge.getSessionTitles(LongArray(0))?.toList())
    }

    @Test
    fun `feedTerminal OSC52 write surfaces clipboard poll event`() {
        withSession { sessionId ->
            // Ghostty→FFI→JSON 全链（host 可验，无需模拟器）：feedTerminal 直注
            // 解析器，on_clipboard_write 回调经 poll_clipboard 由 Event::Clipboard
            // 以 JSON 报出。Kotlin 侧解析由 PollEventTest 覆盖，落盘由复制链覆盖。
            val marker = "SMOKE52_${System.currentTimeMillis() % 100000}"
            val encoded =
                java.util.Base64
                    .getEncoder()
                    .encodeToString(marker.toByteArray(Charsets.UTF_8))
            NativeBridge.feedTerminal(sessionId, "\u001B]52;c;$encoded\u0007".toByteArray(Charsets.UTF_8))
            val seen =
                awaitTrue("clipboard poll event") {
                    generateSequence { NativeBridge.pollEvent() }.take(50).any { json ->
                        json.contains("clipboard") && json.contains(marker)
                    }
                }
            assertTrue("OSC52 write must surface as clipboard poll event", seen)
        }
    }

    @Test
    fun `feedTerminal text is queryable via getTerminalText`() {
        withSession { sessionId ->
            NativeBridge.feedTerminal(sessionId, "hello jni roundtrip".toByteArray())
            val applied =
                awaitTrue("text visible") {
                    val text = NativeBridge.getTerminalText(sessionId)
                    text != null && text.contains("hello jni roundtrip")
                }
            assertTrue("fed text must appear in terminal state", applied)
        }
    }

    @Test
    fun `scrollback rows grows after scrollback-generating output`() {
        withSession { sessionId ->
            assertEquals("fresh session has no scrollback", 0, NativeBridge.scrollbackLength(sessionId))
            // More lines than the visible grid -> pushed into scrollback.
            val payload = (1..200).joinToString("\n") { "scrollback line $it" } + "\n"
            NativeBridge.feedTerminal(sessionId, payload.toByteArray())
            val grew =
                awaitTrue("scrollback populated") {
                    NativeBridge.scrollbackLength(sessionId) > 0
                }
            assertTrue("200 fed lines must create scrollback rows", grew)
        }
    }

    @Test
    fun `negative scrollback row reads as null instead of throwing`() {
        withSession { sessionId ->
            assertEquals(null, NativeBridge.scrollbackLine(sessionId, -1))
        }
    }

    @Test
    fun `unknown session id throws IllegalStateException`() {
        // A bogus id must not crash the process: the export throws a Java
        // exception (jni_export_guard) instead of aborting. The exact type is
        // part of the Kotlin-side contract: `Bridge.onSession` catches **only**
        // IllegalStateException (ffi.rs throw_session_missing), so widening it to a
        // supertype here would silently turn every missing-session call into an
        // opaque propagated exception instead of the intended "session is gone".
        val exception =
            try {
                NativeBridge.getTitle(999_999L)
                null
            } catch (exception: IllegalStateException) {
                exception
            }
        assertNotNull(
            "unknown session must throw IllegalStateException, got silent return",
            exception,
        )
        assertNotNull("bridge must remain usable after the failed call", NativeBridge.listSessions().isEmpty())
    }

    @Test
    fun `JNI call overhead stays in a sane ballpark`() {
        // 探针 MUST 是「真实调用一次」的成本，不是一个常量返回：后者只量到
        // 绑桩跳转，量不到注册表读锁与数组拷贝，回归就藏得住。
        withSession { sessionId ->
            val iterations = 200
            val elapsed =
                measureTimeMillis {
                    repeat(iterations) { NativeBridge.listSessions() }
                }
            println("JNI overhead diagnostic: $iterations listSessions calls took ${elapsed}ms")
            assertTrue(
                "$iterations JNI calls took ${elapsed}ms — regression in bridge cost",
                elapsed < 50,
            )
            // 同一窗口内必须仍能读到刚建的会话：探针没在破坏桥接。
            assertTrue(NativeBridge.listSessions().contains(sessionId))
        }
    }

    /**
     * 跨语言真值测试：Kotlin 的 sp→px 钳位区间 MUST 就是原生 `setRasterScale` 守卫
     * 用的那个区间。
     *
     * 为什么必须经 JNI 读回而不是各写一份字面量：这两份字面量（Kotlin 0.5f..8f、
     * Rust 0.5..=8.0）此前只由注释互相绑定，测试又抄了第三份，于是改任何一处都
     * 不会让任何用例变红——而漂移的后果与字号上界那次同型：钳位点落在原生会拒收的
     * 区间外，字号上界随之用错系数算出。现在区间由 [NativeBridge.getRasterScaleRange]
     * 从原生常量导出，两端共用一份，本用例钉住导出值本身可用、且生产用的换算函数
     * 在任何密度/系统字体缩放组合下都落进它。
     */
    @Test
    fun `the raster scale range reported by the native side is usable and contains every clamped scale`() {
        val bounds = requireNotNull(NativeBridge.getRasterScaleRange()) { "原生未返回光栅缩放区间" }
        assertEquals("原生必须报告两个端点", 2, bounds.size)
        val range = bounds[0]..bounds[1]
        assertTrue("下界必须为正且有限，实际 ${bounds[0]}", bounds[0].isFinite() && bounds[0] > 0f)
        assertTrue("上界必须不小于下界，实际 $bounds", bounds[1].isFinite() && bounds[1] >= bounds[0])

        listOf(0.1f, 0.5f, 1f, 2.625f, 3f, 4f, 8f, 16f, 100f).forEach { density ->
            listOf(0.1f, 0.5f, 1f, 1.3f, 2f, 4f, 10f).forEach { fontScale ->
                val scale = terminal.emulator.runtime.coerceSpToPxScale(density, fontScale, range)
                assertTrue(
                    "density=$density fontScale=$fontScale 得到 $scale，超出原生区间 $range",
                    scale in range,
                )
            }
        }
    }

    /**
     * 跨层覆盖面证明：原生字号上界 MUST 覆盖 Kotlin 的可选上界。
     *
     * Rust 侧 `cap_never_rejects_a_selectable_font_size` 用 **Termux 上界**
     * （256px 换算）代表滑块上界，而 Kotlin 的上界实际是
     * `min(Termux 上界, 「至少 MIN_USABLE_COLUMNS 列」).coerceAtLeast(自适应默认值)`。
     * 只要 Termux 上界在原生全系数区间恒不小于 `ADAPTIVE_DEFAULT_MAX_SP`，那次判定
     * 就是全覆盖而非抽样乐观——这正是本用例逐采样钉住的事实。
     *
     * 放在这里而不是 Rust 侧：这需要同时读 Kotlin 的私有策略常量与经
     * [NativeBridge.getRasterScaleRange] 导出的原生区间端点。Rust 侧原先抄了一份
     * `ADAPTIVE_DEFAULT_MAX_SP = 24.0`，Kotlin 改 24→28 时那边仍全绿，
     * 覆盖面证明静默失效——与本仓要消除的「两份副本」同型。
     *
     * 原生上界 = 图集边长 ÷ 系数，图集边长经原生导出不可得，故用 Rust 侧已钉住的
     * 字面值关系 `cap = ATLAS_SIZE / scale`（`ATLAS_SIZE = 2048`，见
     * `font_size_cap_is_the_atlas_edge_expressed_in_sp` 的断言）反推。
     */
    @Test
    fun `the native font size cap covers every selectable size over the whole scale range`() {
        val bounds = requireNotNull(NativeBridge.getRasterScaleRange()) { "原生未返回光栅缩放区间" }
        val atlasSize = 2048f
        val samples = 64
        for (index in 0..samples) {
            val scale = bounds[0] + (bounds[1] - bounds[0]) * index / samples
            val nativeCapSp = atlasSize / scale
            val kotlinSelectableMaxSp =
                SettingsRepository.fontSizeMaxSp(
                    scale,
                    // 屏宽取 0：列数那条给不出任何上界，正是测试兜底路径的极端情形
                    0f,
                )
            assertTrue(
                "系数=$scale 时原生上界 ${nativeCapSp}sp 覆盖不了 Kotlin 可选上界 " +
                    "${kotlinSelectableMaxSp}sp",
                kotlinSelectableMaxSp <= nativeCapSp,
            )
        }
    }

    /**
     * 回归钉：硬件按键的字节走宿主排队钩子，而不是直写 `feedPty`。
     *
     * 直写的 `feedPty` 在主端满时最多等 `WRITE_DRAIN_TIMEOUT`（5s），而击键来自
     * UI 线程——子进程不读 stdin 时每次按键冻住主线程。宿主把 `ptyWriteQueue` 接到
     * 批缓冲上后，`processKeyEvent` 编码出的字节必须交给该钩子。
     *
     * 判别力：本例断言钩子被调用并收到**非空**字节；还原成直写（钩子恒不被 consult）
     * 时它一次都不会被调用，断言即失败。
     */
    @Test
    fun `encoded key bytes go through the host write queue`() {
        val bridge = TestBridges.create()
        val queued = java.util.concurrent.CopyOnWriteArrayList<ByteArray>()
        bridge.ptyWriteQueue = { sessionId, data ->
            assertTrue("queued write must carry a session id", sessionId > 0)
            queued.add(data)
        }
        val sessionId = bridge.spawnTerminal(24, 80, HOST_SHELL)
        assertTrue("spawnTerminal must return a positive session id", sessionId > 0)
        try {
            // KEYCODE_A：上游 `key::Encoder` 覆盖它，故走 `encoded.isNotEmpty()` 那一支。
            val handled =
                bridge.processKeyEvent(
                    keyCode = android.view.KeyEvent.KEYCODE_A,
                    modifiers = 0,
                    action = android.view.KeyEvent.ACTION_DOWN,
                    unicodeChar = 'a'.code,
                    unshiftedChar = 'a'.code,
                )
            assertTrue("A must be encoded and accepted", handled)
            assertTrue("the queue must receive the encoded bytes", queued.isNotEmpty())
            assertEquals("A encodes to a single byte", 1, queued[0].size)
            assertEquals('a'.code.toByte(), queued[0][0])
        } finally {
            bridge.close()
        }
    }

    /**
     * OSC 7 的工作目录经 JNI 回报：`pwd` 与批量版必须同源，且批量版按实参顺序。
     *
     * 顺序断言有判别力：会话抽屉按自己的会话表回填，若原生按注册表顺序返回，
     * 会话一多就会把目录写到别的会话标题下。
     */
    @Test
    fun `getSessionPwds tracks OSC 7 in the requested order`() {
        withSession { firstId ->
            withSession { secondId ->
                // 任何 OSC 7 之前是空串而不是 null：会话活着但没有目录可报。
                assertEquals("", NativeBridge.getPwd(firstId))
                NativeBridge.feedTerminal(firstId, "\u001b]7;file://host/tmp/first\u0007".toByteArray())
                NativeBridge.feedTerminal(secondId, "\u001b]7;file://host/tmp/second\u0007".toByteArray())
                assertTrue(
                    "两个工作目录都必须被 VT 线程应用",
                    awaitTrue("pwd applied") {
                        NativeBridge.getPwd(firstId)?.endsWith("/tmp/first") == true &&
                            NativeBridge.getPwd(secondId)?.endsWith("/tmp/second") == true
                    },
                )
                assertEquals(
                    listOf("/tmp/first", "/tmp/second"),
                    NativeBridge
                        .getSessionPwds(longArrayOf(firstId, secondId))
                        ?.toList()
                        ?.map { it?.substringAfter("file://host") },
                )
                assertEquals(
                    listOf("/tmp/second", "/tmp/first"),
                    NativeBridge
                        .getSessionPwds(longArrayOf(secondId, firstId))
                        ?.toList()
                        ?.map { it?.substringAfter("file://host") },
                )
                // 未知会话是 null：调用方据此区分「还没有目录」与「会话没了」。
                assertEquals(
                    listOf("/tmp/first", null),
                    NativeBridge
                        .getSessionPwds(longArrayOf(firstId, 999_999L))
                        ?.toList()
                        ?.map { it?.substringAfter("file://host") },
                )
            }
        }
    }

    /** `getPwd` 对未知会话抛 `IllegalStateException`（与其余按会话查询一致）。 */
    @Test
    fun `getPwd throws on an unknown session`() {
        withSession { _ ->
            try {
                NativeBridge.getPwd(999_999L)
                org.junit.Assert.fail("unknown session must throw")
            } catch (expected: IllegalStateException) {
                assertNotNull(expected.message)
            }
        }
    }

    /**
     * 选词（`selectWordAt`）经 JNI 的往返：派生词界 → 装进终端 → 按返回的界限取回该词。
     *
     * Rust 侧 `select_word_at_derives_installs_and_returns_bounds` 只覆盖了内部调用；
     * 这一层要验的是 JNI 边界本身——坐标的钳位、界限数组的打包顺序
     * （`[startRow, startCol, endRow, endCol]`），以及「选区真的装进了终端」：
     * 只回传界限而没安装的话，界面上的反白就没了，而界限看上去仍然正确。
     *
     * 落点不写死：shell 启动时可能先打印提示符，字面量坐标会落到别的词上。
     * 行号从 `getTerminalText` 的行序取（它逐行对应网格行），列号从
     * `scrollbackLine` 取（那一行**每列恰好一个字符**，字符下标即列号）。
     */
    @Test
    fun `selectWordAt derives the word bounds and installs the selection`() {
        withSession { sessionId ->
            val marker = "jniword"
            NativeBridge.feedTerminal(sessionId, "hello $marker world".toByteArray())
            val applied =
                awaitTrue("text visible") {
                    NativeBridge.getTerminalText(sessionId)?.contains(marker) == true
                }
            assertTrue("fed text must land before selecting", applied)

            val lines = NativeBridge.getTerminalText(sessionId)!!.split("\n")
            val row = lines.indexOfFirst { it.contains(marker) }
            assertTrue("the marker must sit on a known row", row >= 0)
            val column =
                requireNotNull(NativeBridge.scrollbackLine(sessionId, row)) {
                    "the marker row must be readable"
                }.indexOf(marker)
            assertTrue("the marker must be found in its own row", column >= 0)

            val bounds =
                requireNotNull(NativeBridge.selectWordAt(sessionId, row, column)) {
                    "a word cell must yield bounds"
                }
            assertEquals("bounds are [startRow, startCol, endRow, endCol]", 4, bounds.size)
            assertEquals(row, bounds[0])
            assertEquals(column, bounds[1])
            assertEquals("the word sits on one row", row, bounds[2])
            assertEquals("the word ends at its last cell", column + marker.length - 1, bounds[3])
            assertEquals(
                "the installed selection must extract exactly the word",
                marker,
                NativeBridge.selectionText(sessionId, bounds[0], bounds[1], bounds[2], bounds[3]),
            )
        }
    }

    /**
     * 空白格没有词可选、未知会话没有选区：两种情况都是 null，而不是一对零界限。
     *
     * 返回零界限会让界面把「没选中」画成「选中了第 0 格第 0 列」。
     */
    @Test
    fun `selectWordAt yields nothing on a blank cell and on an unknown session`() {
        withSession { sessionId ->
            NativeBridge.feedTerminal(sessionId, "hello".toByteArray())
            val applied =
                awaitTrue("text visible") {
                    NativeBridge.getTerminalText(sessionId)?.contains("hello") == true
                }
            assertTrue("fed text must land before selecting", applied)

            val blankRow = 20
            assertNull(
                "row $blankRow must be empty for this case",
                NativeBridge.scrollbackLine(sessionId, blankRow),
            )
            assertNull("a blank cell has no word", NativeBridge.selectWordAt(sessionId, blankRow, 0))
            assertNull("an unknown session has no selection", NativeBridge.selectWordAt(999_999L, 0, 0))
        }
    }

    /**
     * `setCursorStyle` 的入参校验在 JNI 边界上完成。
     *
     * 越界值若被静默接受，会一路走到 `CursorStyle` 映射处才落空，表现为「设置了
     * 光标样式但外观不变」——而日志里只有一行来自渲染线程的警告。
     */
    @Test
    fun `cursor style validates its arguments`() {
        withSession { sessionId ->
            NativeBridge.setCursorStyle(sessionId, 2)
            try {
                NativeBridge.setCursorStyle(sessionId, 3)
                org.junit.Assert.fail("style 3 is out of range")
            } catch (expected: IllegalArgumentException) {
                assertNotNull(expected.message)
            }
        }
    }
}
