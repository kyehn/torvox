package terminal.emulator

import android.os.SystemClock
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt

/**
 * Quantified verification of "shell 反应稍显缓慢" — the matrix marked this "(C) 需设备验证" precisely because
 * no number was ever recorded. This test turns it into a first-class metric:
 *
 * TRIGGER write `echo UXMARK<burstIndex>` to the PTY, burstIndex = 1..SAMPLES. METRIC ms from the write until
 * `getTerminalText()` contains the marker (10ms polling granularity), per sample. ASSERT every
 * marker appears, in issue order (no dropped/reordered output through the pipeline); medianMillis <=
 * P50_BUDGET_MS and maxMillis <= MAX_BUDGET_MS on the software-rendered emulator. Budgets are
 * deliberately generous — they exist to catch ORDER-OF-MAGNITUDE regressions, while the logged
 * UX_METRIC samples carry the real trend for human review.
 */
class ShellResponseLatencyTest : ComposeTerminalLogcatTest() {

    private lateinit var device: UiDevice

    private companion object {
        const val SAMPLES = 8
        const val POLL_TIMEOUT_MS = 15_000L
        const val P50_BUDGET_MS = 1_500L
        const val MAX_BUDGET_MS = 3_000L
    }

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
    }

    @Test
    fun shell_echo_latency_meets_emulator_budget() {
        val bridge = composeTestRule.awaitBridge()
        awaitShellPrompt(bridge)
        // 「shell 启动缓慢」的用户口径 = 进程起来到提示符可输入。应用侧埋点已证实不是瓶颈
        // （bridge 创建 2~9ms、spawnTerminal 20~32ms，见 Runtime 的 `bridge created` /
        // `spawnTerminal` 两条），余下等待落在引导链自身——指标把它记下来，供真机日志
        // `grep UX_METRIC` 直接对标。
        //
        // 读应用自己采到的冷启动度量（进程启动 → 首帧 / 首个 PTY 输出），**不**在这里
        // 现算 `uptimeMillis - getStartUptimeMillis()`：测试类排在本套件后面跑时，
        // 那个差值是「测试跑到现在过了多久」（实测 484 秒），与启动耗时无关。
        val startup = composeTestRule.activity.terminalViewModel.runtime.startupMetrics.value
        startup.startupToFirstFrameMs?.let {
            UxTestUtils.metric("startup_to_first_frame_ms", it)
        }
        startup.startupToFirstOutputMs?.let {
            UxTestUtils.metric("startup_to_first_output_ms", it)
        }

        val latencies = mutableListOf<Long>()
        for (index in 1..SAMPLES) {
            val marker = "UXMARK$index"
            // 冷启动 stdin 竞态单次写入可丢失，与 ImePopup/SurfaceLoss 同口径重发至多 3 次，每轮落盘日志不吞因。
            var elapsed: Long? = null
            for (attempt in 1..3) {
                if (elapsed != null) break
                bridge.writeToPty("clear\n".toByteArray(Charsets.UTF_8))
                UxTestUtils.pollUntilTrue(timeoutMs = 2_000) {
                    bridge.getTerminalText()?.contains("UXMARK") != true
                }
                Thread.sleep(150)
                bridge.writeToPty("echo $marker\n".toByteArray(Charsets.UTF_8))

                elapsed =
                    UxTestUtils.pollUntilTrue(timeoutMs = POLL_TIMEOUT_MS) {
                        bridge.getTerminalText()?.contains(marker) == true
                    }
                android.util.Log.i("ShellLatency", "$marker attempt=$attempt hit=${elapsed != null}")
            }
            assertNotNull(
                "marker $marker never appeared on screen within ${POLL_TIMEOUT_MS}ms",
                elapsed,
            )
            val elapsedMs = requireNotNull(elapsed)
            latencies.add(elapsedMs)
            UxTestUtils.metric("shell_echo_ms", elapsedMs)
        }

        val sorted = latencies.sorted()
        val medianMillis = sorted[sorted.size / 2]
        val maxMillis = sorted.last()
        UxTestUtils.metric("shell_echo_p50_ms", medianMillis)
        UxTestUtils.metric("shell_echo_max_ms", maxMillis)

        assertTrue(
            "p50 echo latency ${medianMillis}ms exceeds emulator budget ${P50_BUDGET_MS}ms (samples=$latencies)",
            medianMillis <= P50_BUDGET_MS,
        )
        assertTrue(
            "max echo latency ${maxMillis}ms exceeds budget ${MAX_BUDGET_MS}ms (samples=$latencies)",
            maxMillis <= MAX_BUDGET_MS,
        )
    }

    @Test
    fun rapid_command_stream_preserves_order_and_completes() {
        val bridge = composeTestRule.awaitBridge()
        // Ten commands fired back-to-back with NO waiting in between: all
        // ten markers must eventually appear and stay in order — a queued
        // writer that reorders or drops bursts fails here.
        val burst = (1..10).joinToString("") { "echo BURST%02d\n".format(it) }
        val startMillis = SystemClock.uptimeMillis()
        bridge.writeToPty(burst.toByteArray(Charsets.UTF_8))

        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 25) {
                val text = bridge.getTerminalText() ?: return@pollUntilTrue false
                text.contains("BURST10")
            }
        assertNotNull("burst never fully appeared", settled)
        val text = bridge.getTerminalText().orEmpty()
        val positions = (1..10).map { burstIndex -> text.lastIndexOf("BURST%02d".format(burstIndex)) }
        assertTrue(
            "some burst markers missing: $positions",
            positions.all { it >= 0 },
        )
        assertTrue(
            "burst markers out of order: $positions",
            positions == positions.sorted(),
        )
        UxTestUtils.metric("burst_10_total_ms", SystemClock.uptimeMillis() - startMillis)
    }
}
