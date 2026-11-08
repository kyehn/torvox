package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * [RenderErrorLedger]：渲染循环失败诊断的窗口节流。
 *
 * 判别力：节流若挂在「连续错误数」上，成功一帧就把连续数清零——失败与成功交替的
 * 形态下每次都命中首条分支，于是逐帧记一条 error（还带完整栈）。用例把交替形态
 * 摆在同一条时间轴上，断言一个窗口内只出一行；同时钉住四类诊断各持一把闸门
 * （共用会让先到的那类吞掉后到的次数，规范明令禁止）。
 */
class RenderErrorLedgerTest {
    private companion object {
        /** 窗口长度：与 [CountedLogThrottle] 的默认窗口同值。 */
        const val WINDOW_NANOS = 500_000_000L
    }

    /**
     * 交替形态只出一行：失败一帧、成功一帧地交替一百次，连续错误数每次都回到 0，
     * 按连续数节流的旧实现每次都记一行。
     */
    @Test
    fun `alternating failure and recovery produce one line per window`() {
        val ledger = RenderErrorLedger()
        var lines = 0
        var now = 1_000L
        repeat(100) {
            ledger.onTransientError(now)?.let { lines++ }
            ledger.onRecovery(now)?.let { lines++ }
            ledger.onSuccess()
            now += 1_000L
        }
        assertEquals("100 次交替只应出首条两行", 2, lines)
    }

    /** 窗口内的事件不得丢失：下一次放行时必须把攒下的次数一并交出。 */
    @Test
    fun `the window hands over the accumulated count`() {
        val ledger = RenderErrorLedger()
        assertEquals(1, ledger.onTransientError(1_000L))
        assertNull(ledger.onTransientError(1_100L))
        assertNull(ledger.onTransientError(1_200L))
        assertEquals(3, ledger.onTransientError(1_000L + WINDOW_NANOS))
    }

    /** 四类诊断各持一把闸门：共用会让先到的那类吞掉后到的次数。 */
    @Test
    fun `each failure kind has its own gate`() {
        val ledger = RenderErrorLedger()
        assertEquals(1, ledger.onTransientError(1_000L))
        assertEquals(1, ledger.onRenderException(1_000L))
        assertEquals(1, ledger.onRecovery(1_000L))
        assertEquals(1, ledger.onPollFailure(1_000L))
    }

    /** 连续错误数：失败累加，成功清零（退避档位与致命上限靠它）。 */
    @Test
    fun `the consecutive error count resets on success`() {
        val ledger = RenderErrorLedger()
        ledger.onTransientError(1_000L)
        ledger.onRenderException(1_100L)
        assertEquals(2, ledger.consecutiveErrors)
        ledger.onSuccess()
        assertEquals(0, ledger.consecutiveErrors)
    }
}
