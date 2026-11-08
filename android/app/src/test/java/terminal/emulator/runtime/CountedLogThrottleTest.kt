package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * 带计数的限频闸门：逐帧诊断按窗口汇总输出，窗口内的事件计数不得丢失。
 *
 * 判别力：闸门若只丢弃不计数，排障时看不出「多久一次、一次多少」；若计数而不
 * 限频，logcat 会被逐帧诊断刷满。两者必须同时成立。
 */
class CountedLogThrottleTest {
    private companion object {
        /** 窗口长度：与实现取同一个常量，不另存一份。 */
        const val INTERVAL_NANOS = 500_000_000L
    }

    /** 首条必须可见：诊断的第一条往往就是最关键的那条（首次出现的慢帧）。 */
    @Test
    fun theFirstEventIsAlwaysReported() {
        val throttle = CountedLogThrottle()
        assertEquals("首条必须带窗口计数交出", 1, throttle.record(atNanos = 1_000L))
    }

    /** 窗口内的后续事件必须被压住——这正是「逐帧刷屏」的形态。 */
    @Test
    fun eventsInsideTheWindowAreSuppressed() {
        val throttle = CountedLogThrottle()
        throttle.record(atNanos = 1_000L)
        for (atNanos in listOf(1_001L, 100_000L, INTERVAL_NANOS - 1L)) {
            assertNull("atNanos=$atNanos 落在窗口内，必须压住", throttle.record(atNanos = atNanos))
        }
    }

    /** 限频吞掉的是条数，不是次数：窗口内的计数须攒着下次一并交出。 */
    @Test
    fun suppressedEventsAreCountedAndHandedOverLater() {
        val throttle = CountedLogThrottle()
        assertEquals(1, throttle.record(atNanos = 1_000L))
        assertNull(throttle.record(atNanos = 1_100L))
        assertNull(throttle.record(atNanos = 1_200L))
        assertEquals(2, throttle.pendingCount)
        assertEquals(
            "窗口内累计 3 次（首条 + 2 次压住）必须原样交出",
            3,
            throttle.record(atNanos = 1_000L + INTERVAL_NANOS),
        )
        assertEquals("交出后不得把上一窗口的次数算进来", 0, throttle.pendingCount)
    }

    /** 时钟回跳不得放行：回跳会算出负间隔，按「差值小于窗口」判定会被永久放行。 */
    @Test
    fun aClockGoingBackwardsDoesNotUnlockTheWindow() {
        val throttle = CountedLogThrottle()
        throttle.record(atNanos = 10_000_000L)
        assertNull("时钟回跳不得放行", throttle.record(atNanos = 9_000_000L))
        assertNull("时钟回跳不得放行", throttle.record(atNanos = 5_000_000L))
    }
}
