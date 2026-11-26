package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * vsync 信号链：帧推进、停链判定与显示周期实测。
 *
 * 判别力：链常驻的代价是主线程按显示帧率被唤醒，故必须钉住「无事可做即停链」；
 * 显示周期写死会把刷新率降频误读成渲染故障，故必须由相邻两帧实测。
 */
class VsyncChainTest {
    /** 链的外部世界：唤醒记录、会话状态与前后台。 */
    private class Harness {
        val frameClock = FakeFrameClock()

        /** 每帧的唤醒时刻。 */
        val wakeCalls = mutableListOf<Long>()

        /** 唤醒时的钩子：用于模拟「判定与停链之间刚好来了新信号」。 */
        var onWake: () -> Unit = {}

        var hasSessions = true
        var anySessionActive = true
        var appForeground = true

        val chain =
            VsyncChain(
                postToMain = { action -> action() },
                frameClock = frameClock,
                wakeActiveSession = { frameTimeNanos ->
                    wakeCalls += frameTimeNanos
                    onWake()
                },
                hasSessions = { hasSessions },
                anySessionActive = { anySessionActive },
                appForeground = { appForeground },
            )
    }

    @Test
    fun `the chain wakes the active session on every frame`() {
        val harness = Harness()
        harness.chain.ensureRunning()

        harness.frameClock.step(frameTimeNanos = 1_000_000L)

        assertEquals("每个显示帧都要唤醒一次渲染线程", listOf(1_000_000L), harness.wakeCalls)
    }

    @Test
    fun `the chain stays scheduled while a session is active`() {
        val harness = Harness()
        harness.chain.ensureRunning()

        harness.frameClock.step()

        assertTrue("有活跃会话时链必须继续", harness.frameClock.hasQueued())
    }

    @Test
    fun `the chain stops when no session is left`() {
        val harness = Harness()
        harness.hasSessions = false
        harness.chain.ensureRunning()

        harness.frameClock.step()

        assertFalse("无会话时链必须停掉", harness.frameClock.hasQueued())
    }

    @Test
    fun `the chain stops while the app is in the background`() {
        val harness = Harness()
        harness.appForeground = false
        harness.chain.ensureRunning()

        harness.frameClock.step()

        assertFalse("应用不可见时链必须停掉", harness.frameClock.hasQueued())
    }

    @Test
    fun `the chain restarts when a session becomes active right after the stop decision`() {
        val harness = Harness()
        harness.anySessionActive = false
        // 唤醒时来了新信号：判定之后会话重新活跃。
        harness.onWake = { harness.anySessionActive = true }
        harness.chain.ensureRunning()

        harness.frameClock.step()

        assertTrue("停链复核发现会话活跃时必须重新拉起", harness.frameClock.hasQueued())
    }

    @Test
    fun `the display period is measured from consecutive frames`() {
        val harness = Harness()
        harness.chain.ensureRunning()

        harness.frameClock.step(frameTimeNanos = 1_000_000L)
        harness.frameClock.step(frameTimeNanos = 17_000_000L)

        assertTrue(
            "显示周期必须由相邻两帧实测得出",
            harness.chain.displayPeriodText().startsWith("16.0ms"),
        )
    }
}
