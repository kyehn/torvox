package terminal.emulator

import android.os.SystemClock
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

/**
 * Quantified verification of "shell 反应稍显缓慢" — the matrix marked this "(C) 需设备验证" precisely because
 * no number was ever recorded. This test turns it into a first-class metric:
 *
 * TRIGGER write `echo UXMARK<i>` to the PTY, i = 1..SAMPLES. METRIC ms from the write until
 * `getTerminalText()` contains the marker (10ms polling granularity), per sample. ASSERT every
 * marker appears, in issue order (no dropped/reordered output through the pipeline); p50 <=
 * P50_BUDGET_MS and max <= MAX_BUDGET_MS on the software-rendered emulator. Budgets are
 * deliberately generous — they exist to catch ORDER-OF-MAGNITUDE regressions, while the logged
 * UX_METRIC samples carry the real trend for human review.
 */
class ShellResponseLatencyTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    private companion object {
        const val SAMPLES = 8
        const val POLL_TIMEOUT_MS = 4_000L
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
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

        val latencies = mutableListOf<Long>()
        for (i in 1..SAMPLES) {
            val marker = "UXMARK$i"
            // Drain any earlier marker so the predicate matches THIS one.
            bridge.writeToPty("clear\n".toByteArray(Charsets.UTF_8))
            UxTestUtils.pollUntilTrue(timeoutMs = 2_000) {
                bridge.getTerminalText()?.contains("UXMARK") != true
            }
            Thread.sleep(150)

            val elapsed =
                UxTestUtils.pollUntilTrue(timeoutMs = POLL_TIMEOUT_MS) {
                    bridge.getTerminalText()?.contains(marker) == true
                }
            assertNotNull(
                "marker $marker never appeared on screen within ${POLL_TIMEOUT_MS}ms",
                elapsed,
            )
            latencies.add(elapsed!!)
            UxTestUtils.metric("shell_echo_ms", elapsed)
        }

        val sorted = latencies.sorted()
        val p50 = sorted[sorted.size / 2]
        val max = sorted.last()
        UxTestUtils.metric("shell_echo_p50_ms", p50)
        UxTestUtils.metric("shell_echo_max_ms", max)

        assertTrue(
            "p50 echo latency ${p50}ms exceeds emulator budget ${P50_BUDGET_MS}ms (samples=$latencies)",
            p50 <= P50_BUDGET_MS,
        )
        assertTrue(
            "max echo latency ${max}ms exceeds budget ${MAX_BUDGET_MS}ms (samples=$latencies)",
            max <= MAX_BUDGET_MS,
        )
    }

    @Test
    fun rapid_command_stream_preserves_order_and_completes() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        // Ten commands fired back-to-back with NO waiting in between: all
        // ten markers must eventually appear and stay in order — a queued
        // writer that reorders or drops bursts fails here.
        val burst = (1..10).joinToString("") { "echo BURST%02d\n".format(it) }
        val t0 = SystemClock.uptimeMillis()
        bridge.writeToPty(burst.toByteArray(Charsets.UTF_8))

        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = 6_000, intervalMs = 25) {
                val text = bridge.getTerminalText() ?: return@pollUntilTrue false
                text.contains("BURST10")
            }
        assertNotNull("burst never fully appeared", settled)
        val text = bridge.getTerminalText().orEmpty()
        val positions = (1..10).map { i -> text.lastIndexOf("BURST%02d".format(i)) }
        assertTrue(
            "some burst markers missing: $positions",
            positions.all { it >= 0 },
        )
        assertTrue(
            "burst markers out of order: $positions",
            positions == positions.sorted(),
        )
        UxTestUtils.metric("burst_10_total_ms", SystemClock.uptimeMillis() - t0)
    }
}
