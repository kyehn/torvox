package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 慢循环判定口径：主动等待（空闲门控驻留、渲染失败退避）不计入工作时长。
 *
 * 判别力：真机日志里「平台收走原生窗口 → 渲染失败 → 50ms 退避」这条正常退化路径
 * 曾被判成慢循环并记出 warn 级告警（8.5 分钟内 5 条应用自有警告全部来自这条误报，
 * 而真实渲染耗时只有 1–5ms）。判定若把退避算进去，这些误报会一直存在。
 */
class SlowLoopWorkNanosTest {
    private companion object {
        /** 判定阈值换算成纳秒：与实现取同一个常量，不另存一份。 */
        const val THRESHOLD_NANOS = SLOW_FRAME_LOG_THRESHOLD_MS * 1_000_000L
    }

    /**
     * 真机实测数值：整循环 55.6ms，其中退避 50.5ms、真实工作约 5.1ms。
     *
     * 退避是失败后的刻意等待，不得计入工作时长，也不得让整次循环被判成慢。
     */
    @Test
    fun transientErrorBackoffIsNotCountedAsSlowWork() {
        val workNanos =
            slowLoopWorkNanos(
                totalNanos = 55_600_000L,
                gateNanos = 0L,
                backoffNanos = 50_500_000L,
            )
        assertEquals("退避必须从工作时长中扣除", 5_100_000L, workNanos)
        assertFalse(
            "50ms 退避 + 5ms 工作不得判成慢循环",
            workNanos > THRESHOLD_NANOS,
        )
    }

    /** 空闲门控驻留同样是主动等待，不计入判定：空终端因此不会永久告警。 */
    @Test
    fun idleLatchWaitIsNotCountedAsSlowWork() {
        val workNanos =
            slowLoopWorkNanos(
                totalNanos = 501_000_000L,
                gateNanos = 500_000_000L,
                backoffNanos = 0L,
            )
        assertEquals("门控驻留必须从工作时长中扣除", 1_000_000L, workNanos)
        assertFalse("500ms 门控驻留 + 1ms 工作不得判成慢循环", workNanos > THRESHOLD_NANOS)
    }

    /** 真实慢渲染照常计入判定：45ms 的渲染在 30ms 阈值之上。 */
    @Test
    fun realSlowRenderIsStillCountedAsSlowWork() {
        val workNanos =
            slowLoopWorkNanos(
                totalNanos = 45_500_000L,
                gateNanos = 0L,
                backoffNanos = 0L,
            )
        assertEquals("无主动等待时工作时长即整循环时长", 45_500_000L, workNanos)
        assertTrue("45ms 的真实慢渲染必须仍被判成慢循环", workNanos > THRESHOLD_NANOS)
    }
}
