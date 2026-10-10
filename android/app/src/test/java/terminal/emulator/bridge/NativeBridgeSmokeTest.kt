package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
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
 * `nix develop --command cargo build --package native --profile release`) and drives the real JNI
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
 * - env.throw_new exception paths (IllegalArgumentException on bad args)
 * - the full Kotlin→JNI→command channel→VT thread→ghostty parser→query round-trip; `vt_write` hands
 *   the buffer to the VT thread via `try_send`, so a broken command channel or dead VT thread only
 *   shows here (previously only the emulator caught it — heavy and black-box)
 *
 * Graphics exports (attachWindow/render/captureFrame) need ANativeWindow and stay on the emulator;
 * everything exercised here is pure CPU logic.
 *
 * Locating the library: unit tests run with cwd = `android/app/`, so the path is the fixed
 * `../../target/release/libnative.so`. No candidate probing and no environment override — the
 * library must already be there (built as above, the step the `check` workflow runs before
 * `check-gradle.nu`), or the load fails loudly.
 */
class NativeBridgeSmokeTest {
    private companion object {
        /** 固定路径：缺失时 System.load 自身抛错，不再探测候选位置。 */
        private const val HOST_LIBRARY_PATH = "../../target/release/libnative.so"

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
        val before = NativeBridge.getSessionCount()
        withSession { sessionId ->
            assertTrue(
                "registry must now contain the session",
                requireNotNull(NativeBridge.listSessions()).contains(sessionId.toString()),
            )
        }
        assertEquals("session must be removed again", before, NativeBridge.getSessionCount())
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
    fun `feedTerminal OSC52 write surfaces clipboard poll event`() {
        withSession { sessionId ->
            // Ghostty→FFI→JSON 全链（host 可验，无需模拟器）：feedTerminal 直注
            // 解析器，on_clipboard_write 回调经 poll_clipboard 由 Event::Clipboard
            // 以 JSON 报出。Kotlin 侧解析由 PollEventTest 覆盖，落盘由复制链覆盖。
            val marker = "SMOKE52_${System.currentTimeMillis() % 100000}"
            val encoded =
                java.util.Base64.getEncoder().encodeToString(marker.toByteArray(Charsets.UTF_8))
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
            assertEquals("fresh session has no scrollback", 0, NativeBridge.getScrollbackRows(sessionId))
            // More lines than the visible grid -> pushed into scrollback.
            val payload = (1..200).joinToString("\n") { "scrollback line $it" } + "\n"
            NativeBridge.feedTerminal(sessionId, payload.toByteArray())
            val grew =
                awaitTrue("scrollback populated") {
                    NativeBridge.getScrollbackRows(sessionId) > 0
                }
            assertTrue("200 fed lines must create scrollback rows", grew)
            assertEquals("unknown session reads as 0 rows", 0, NativeBridge.getScrollbackRows(999_999L))
        }
    }

    @Test
    fun `negative scrollback row reads as null instead of throwing`() {
        withSession { sessionId ->
            assertEquals(null, NativeBridge.scrollbackLine(sessionId, -1))
        }
    }

    @Test
    fun `unknown session id throws IllegalArgumentException`() {
        // A bogus id must not crash the process: the export throws a Java
        // exception (jni_export_guard) instead of aborting. The exact type
        // is part of the Kotlin-side contract (ffi.rs getTitle throws
        // IllegalArgumentException for unknown sessions).
        val exception =
            try {
                NativeBridge.getTitle(999_999L)
                null
            } catch (exception: IllegalArgumentException) {
                exception
            }
        assertNotNull(
            "unknown session must throw IllegalArgumentException, got silent return",
            exception,
        )
        assertNotNull("bridge must remain usable after the failed call", NativeBridge.listSessions())
    }

    @Test
    fun `JNI call overhead stays in a sane ballpark`() {
        withSession { sessionId ->
            val iterations = 200
            val elapsed = measureTimeMillis {
                repeat(iterations) { NativeBridge.getSessionCount() }
            }
            println("JNI overhead diagnostic: $iterations getSessionCount calls took ${elapsed}ms")
            assertTrue(
                "200 JNI calls took ${elapsed}ms — regression in bridge cost",
                elapsed < 50,
            )
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
}
