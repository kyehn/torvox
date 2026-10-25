package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.bridge.TestBridges

/**
 * T3 backspace-latency cadence tests: the render loop must leave the 500ms idle latch (and stay on
 * the 17ms active cadence) whenever the idle clock is fresh — input writes refresh it through
 * [SessionEntry.notifyRender], including the hardware-key / IME-sendKeyEvent path wired via the
 * [terminal.emulator.bridge.Bridge.onPtyWrite] hook. Without that wake a backspace pressed after
 * >5s idle would have its shell echo wait out the full 500ms idle-latch tick.
 *
 * Mirrors the [ShouldResetScrollTest] style: the gate is a pure function plus the refresh contract
 * of the wake signal, both side-effect free and deterministic.
 */
class RenderLatchCadenceTest {
    private companion object {
        /** Synthetic staleness threshold — matches the loop's 5s idle clock at call sites. */
        const val IDLE_THRESHOLD_NANOS = 5_000_000_000L
    }

    private data class Case(val idleNanos: Long, val hasScrollMotion: Boolean, val expected: Boolean)

    /**
     * Full four-combination truth table of the latch gate: stale clock without scroll motion is
     * the ONLY state that selects the idle 500ms latch. A stale clock WITH scroll motion stays on
     * the active cadence (T1), and a fresh clock always stays active (input wake refreshes the
     * clock, so a backspace after idle never parks slow again).
     */
    @Test
    fun allFourCombinationsFollowIdleGate() {
        val cases =
            arrayOf(
                Case(idleNanos = 0L, hasScrollMotion = false, expected = false),
                Case(idleNanos = 0L, hasScrollMotion = true, expected = false),
                Case(idleNanos = IDLE_THRESHOLD_NANOS + 1L, hasScrollMotion = false, expected = true),
                Case(idleNanos = IDLE_THRESHOLD_NANOS + 1L, hasScrollMotion = true, expected = false),
            )
        for (case in cases) {
            assertEquals(
                "idleNanos=${case.idleNanos} hasScrollMotion=${case.hasScrollMotion}",
                case.expected,
                shouldUseIdleLatch(
                    idleNanos = case.idleNanos,
                    hasScrollMotion = case.hasScrollMotion,
                    idleThresholdNanos = IDLE_THRESHOLD_NANOS,
                ),
            )
        }
    }

    /** Boundary: exactly at the threshold the clock is not stale yet → active latch. */
    @Test
    fun clockExactlyAtThresholdStaysActive() {
        assertFalse(
            shouldUseIdleLatch(
                idleNanos = IDLE_THRESHOLD_NANOS,
                hasScrollMotion = false,
                idleThresholdNanos = IDLE_THRESHOLD_NANOS,
            ),
        )
    }

    /**
     * 活跃闭锁不得与 vsync 在同一显示周期各推一帧。
     *
     * 判别力：健康 vsync 下若仍用 17ms 闭锁（≈一个 60Hz 周期），两路节拍的相位漂移会让
     * 闭锁偶尔抢在 vsync 之前触发，同一周期推出两帧；交换链随即排满，下一次 acquire
     * 阻塞约 70ms（真机日志里成对的 3–5ms 呈现间隔 + 70ms 空档）。故健康时必须明显
     * 大于一个 vsync 周期；vsync 停摆时退回 17ms 兜底。
     */
    @Test
    fun activeLatchOutlastsOneVsyncWhenVsyncIsFresh() {
        val fresh = activeLatchTimeoutNanos(vsyncFresh = true)
        assertTrue(
            "健康 vsync 下的活跃闭锁必须大于一个 60Hz 周期（实际 $fresh ns）",
            fresh > 17_000_000L,
        )
        assertEquals(
            "vsync 停摆时必须退回 17ms 兜底节拍",
            RENDER_LATCH_TIMEOUT_NANOS,
            activeLatchTimeoutNanos(vsyncFresh = false),
        )
        assertTrue(
            "vsync 新鲜度阈值必须覆盖多个显示周期",
            RENDER_VSYNC_STALE_NANOS >= 3 * RENDER_LATCH_TIMEOUT_NANOS,
        )
    }

    /**
     * 渲染线程从 vsync 回调取速度：回调刷新 [SessionEntry.lastVsyncNanos]，
     * 循环据此判定链是否健康（陈旧即退回兜底）。
     */
    @Test
    fun vsyncCallbackTimestampMarksTheChainFresh() {
        val entry =
            SessionEntry(id = 1L, bridge = TestBridges.create(), renderThreadRef = null, running = false)
        entry.lastVsyncNanos = System.nanoTime() - RENDER_VSYNC_STALE_NANOS - 1L
        val stale = System.nanoTime() - entry.lastVsyncNanos <= RENDER_VSYNC_STALE_NANOS
        assertFalse("超过阈值的时间戳必须判为不新鲜", stale)
        entry.lastVsyncNanos = System.nanoTime()
        val fresh = System.nanoTime() - entry.lastVsyncNanos <= RENDER_VSYNC_STALE_NANOS
        assertTrue("刚收到的 vsync 必须判为新鲜", fresh)
    }

    /**
     * The wake contract the fix relies on: [SessionEntry.notifyRender] (invoked for every PTY
     * write through the onPtyWrite wiring) refreshes the idle clock and raises the render-signal,
     * so the gate immediately selects the active latch for the incoming shell echo.
     */
    @Test
    fun notifyRenderRefreshesIdleClockAndRaisesSignal() {
        val entry =
            SessionEntry(id = 1L, bridge = TestBridges.create(), renderThreadRef = null, running = false)
        val staleClock = System.nanoTime() - IDLE_THRESHOLD_NANOS - 1L
        entry.lastSignalNanos = staleClock
        entry.renderSignaled.set(false)

        entry.notifyRender()

        assertTrue("input wake must raise the render signal", entry.renderSignaled.get())
        assertTrue(
            "input wake must refresh the idle clock (active latch, not 500ms idle)",
            entry.lastSignalNanos > staleClock,
        )
        val refreshedIdleNanos = System.nanoTime() - entry.lastSignalNanos
        assertFalse(
            "fresh idle clock must select the active latch",
            shouldUseIdleLatch(
                idleNanos = refreshedIdleNanos,
                hasScrollMotion = entry.hasScrollMotion(),
                idleThresholdNanos = IDLE_THRESHOLD_NANOS,
            ),
        )
    }
}
