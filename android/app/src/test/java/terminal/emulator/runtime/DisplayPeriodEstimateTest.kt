package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 显示周期的实测：帧回调时间戳差就是当前显示的实际节拍。
 *
 * 平台不保证刷新率恒定（省电与热节流都会把它压到 60Hz），把 60Hz 的 16.7ms
 * 当作设备的帧预算就会在降频设备上把显示降频误读成渲染故障。故周期由实测得出，
 * 且丢帧造成的长时间间隔不得被当成周期。
 */
class DisplayPeriodEstimateTest {

    private val sixtyHertzPeriodNanos = 16_667_000L
    private val oneHundredTwentyHertzPeriodNanos = 8_333_000L

    @Test
    fun `the first measured interval becomes the period`() {
        val estimate =
            updateDisplayPeriodEstimate(
                estimate = DisplayPeriodEstimate(periodNanos = 0L, sampledAtNanos = 0L),
                intervalNanos = sixtyHertzPeriodNanos,
                nowNanos = 1_000_000_000L,
            )
        assertEquals(sixtyHertzPeriodNanos, estimate.periodNanos)
    }

    @Test
    fun `a dropped frame does not stretch the period`() {
        val first =
            updateDisplayPeriodEstimate(
                estimate = DisplayPeriodEstimate(periodNanos = 0L, sampledAtNanos = 0L),
                intervalNanos = sixtyHertzPeriodNanos,
                nowNanos = 1_000_000_000L,
            )
        val afterDroppedFrame =
            updateDisplayPeriodEstimate(
                estimate = first,
                intervalNanos = 2 * sixtyHertzPeriodNanos,
                nowNanos = 1_010_000_000L,
            )
        assertEquals(
            "丢帧只让间隔变大，周期必须保持窗口内观测到的最小值",
            sixtyHertzPeriodNanos,
            afterDroppedFrame.periodNanos,
        )
    }

    @Test
    fun `a shorter interval replaces the previous period`() {
        val first =
            updateDisplayPeriodEstimate(
                estimate = DisplayPeriodEstimate(periodNanos = 0L, sampledAtNanos = 0L),
                intervalNanos = sixtyHertzPeriodNanos,
                nowNanos = 1_000_000_000L,
            )
        val afterRefreshRateChange =
            updateDisplayPeriodEstimate(
                estimate = first,
                intervalNanos = oneHundredTwentyHertzPeriodNanos,
                nowNanos = 1_010_000_000L,
            )
        assertEquals(oneHundredTwentyHertzPeriodNanos, afterRefreshRateChange.periodNanos)
    }

    @Test
    fun `the window restarts so a slower refresh rate is picked up`() {
        val first =
            updateDisplayPeriodEstimate(
                estimate = DisplayPeriodEstimate(periodNanos = 0L, sampledAtNanos = 0L),
                intervalNanos = oneHundredTwentyHertzPeriodNanos,
                nowNanos = 1_000_000_000L,
            )
        val afterWindow =
            updateDisplayPeriodEstimate(
                estimate = first,
                intervalNanos = sixtyHertzPeriodNanos,
                nowNanos = 1_000_000_000L + DISPLAY_PERIOD_WINDOW_NANOS + 1L,
            )
        assertEquals(
            "刷新率降到 60Hz 后旧的小间隔必须退出窗口",
            sixtyHertzPeriodNanos,
            afterWindow.periodNanos,
        )
    }

    @Test
    fun `an implausible interval is ignored`() {
        val first =
            updateDisplayPeriodEstimate(
                estimate = DisplayPeriodEstimate(periodNanos = 0L, sampledAtNanos = 0L),
                intervalNanos = sixtyHertzPeriodNanos,
                nowNanos = 1_000_000_000L,
            )
        val afterLongGap =
            updateDisplayPeriodEstimate(
                estimate = first,
                intervalNanos = MAX_DISPLAY_PERIOD_NANOS + 1L,
                nowNanos = 1_010_000_000L,
            )
        assertEquals(
            "主线程长时间被占用造成的间隔不是显示周期",
            sixtyHertzPeriodNanos,
            afterLongGap.periodNanos,
        )
    }
}
