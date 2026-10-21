package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FrameTimingStatsTest {
    @Test
    fun `report is null until the window fills`() {
        val stats = FrameTimingStats(windowSize = 3)
        stats.record(1_000_000L)
        stats.record(2_000_000L)
        assertFalse(stats.isWindowComplete())
        assertNull(stats.takeReport())
        stats.record(3_000_000L)
        assertTrue(stats.isWindowComplete())
        val report = checkNotNull(stats.takeReport())
        assertEquals(3, report.frameCount)
    }

    @Test
    fun `report has correct avg p95 and max for a known sequence`() {
        val stats = FrameTimingStats(windowSize = 60)
        val durationsMs = (1..60).map { it * 1_000_000L } // 1ms..60ms
        durationsMs.forEach(stats::record)
        val report = checkNotNull(stats.takeReport())
        assertEquals(60, report.frameCount)
        // sum(1..60) = 1830ms -> 1830_000_000ns / 60 = 30_500_000ns
        assertEquals(30_500_000L, report.averageNanos)
        // p95 index = ceil(0.95*60)-1 = 56 -> value 57ms
        assertEquals(57_000_000L, report.p95Nanos)
        assertEquals(60_000_000L, report.maxNanos)
    }

    @Test
    fun `window resets and a fresh window reports independently`() {
        val stats = FrameTimingStats(windowSize = 2)
        stats.record(5_000_000L)
        stats.record(7_000_000L)
        val first = checkNotNull(stats.takeReport())
        assertEquals(6_000_000L, first.averageNanos)
        // New window: old samples must not leak in.
        stats.record(100_000_000L)
        assertNull(stats.takeReport())
        stats.record(200_000_000L)
        val second = checkNotNull(stats.takeReport())
        assertEquals(150_000_000L, second.averageNanos)
        assertEquals(200_000_000L, second.maxNanos)
        assertEquals(200_000_000L, second.p95Nanos) // n=2: p95 index = ceil(0.95*2)-1 = 1 -> max
    }

    @Test
    fun `frames beyond a full window are dropped instead of overflowing the array`() {
        // 越界写会在**渲染线程**上抛 ArrayIndexOutOfBoundsException，整个终端随之消失；
        // 未先排空窗口就继续记录的那一帧必须被丢弃，且不能污染已满的窗口。
        val stats = FrameTimingStats(windowSize = 2)
        stats.record(4_000_000L)
        stats.record(6_000_000L)
        stats.record(999_000_000L)
        val report = checkNotNull(stats.takeReport())
        assertEquals(2, report.frameCount)
        assertEquals(5_000_000L, report.averageNanos)
        assertEquals(6_000_000L, report.maxNanos)
        // 丢弃的那帧不得让窗口提前就绪。
        assertNull(stats.takeReport())
    }

    @Test
    fun `p95 ignores rare slow frames above the 95th percentile`() {
        val stats = FrameTimingStats(windowSize = 100)
        // 95 fast frames of 4ms, then 5 slow frames of 1s.
        repeat(95) { stats.record(4_000_000L) }
        repeat(5) { stats.record(1_000_000_000L) }
        val report = checkNotNull(stats.takeReport())
        assertEquals(4_000_000L, report.p95Nanos)
        assertEquals(1_000_000_000L, report.maxNanos)
        assertEquals(53_800_000L, report.averageNanos) // (95*4 + 5*1000)/100 ms
    }

    @Test
    fun `window closes on the duration cap when frames are scarce`() {
        // 低帧率设备实测 1.8–8 FPS：拖动 2.5s 只有 3 帧，循环随后转入空闲闭锁不再出新帧。
        // 只按帧数结算的话这 3 帧永远不会被报告，故时长到上限即出报告。
        val stats = FrameTimingStats(windowSize = 60, maxWindowNanos = 2_000_000_000L)
        stats.record(100_000_000L, atNanos = 0L)
        stats.record(200_000_000L, atNanos = 900_000_000L)
        assertFalse("未到 2s 不应提前结算", stats.isWindowComplete())
        stats.record(300_000_000L, atNanos = 2_000_000_000L)
        assertTrue(stats.isWindowComplete())
        val report = checkNotNull(stats.takeReport())
        assertEquals(3, report.frameCount)
        assertEquals(200_000_000L, report.averageNanos)
        assertEquals(300_000_000L, report.maxNanos)
        assertEquals(300_000_000L, report.p95Nanos) // n=3: ceil(0.95*3)-1 = 2 -> max
        // 结算后重新计时，不能带着旧窗口的起点立刻再结算。
        stats.record(50_000_000L, atNanos = 2_100_000_000L)
        assertFalse(stats.isWindowComplete())
    }

    @Test
    fun `fast devices still report on the frame count, not the duration cap`() {
        val stats = FrameTimingStats(windowSize = 3, maxWindowNanos = 2_000_000_000L)
        // 60 FPS：3 帧仅 50ms，远在上限之内。
        stats.record(16_000_000L, atNanos = 0L)
        stats.record(16_000_000L, atNanos = 16_000_000L)
        stats.record(16_000_000L, atNanos = 33_000_000L)
        assertTrue(stats.isWindowComplete())
        assertEquals(3, checkNotNull(stats.takeReport()).frameCount)
    }
}
