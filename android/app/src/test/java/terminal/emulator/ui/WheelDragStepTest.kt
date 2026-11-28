package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 触摸拖动 → 滚轮档位的换算。
 *
 * 断言的是方向与刻度两个独立不变式：手指上移为滚轮上、下移为滚轮下（与鼠标上报
 * 仪器用例一致），刻度必须是「走满一行才发一档」（否则慢速拖动被放大成逐事件一档）。
 */
class WheelDragStepTest {

    @Test
    fun `finger moving down forwards wheel down`() {
        // distanceY 为前一帧减当前帧：手指下移为负。
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = -40f, cellHeightPx = 40f)
        assertEquals(WHEEL_DOWN_BUTTON, step.button)
        assertEquals(1, step.notches)
        assertEquals(0f, step.remainderPx, 0.01f)
    }

    @Test
    fun `finger moving up forwards wheel up`() {
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = 40f, cellHeightPx = 40f)
        assertEquals(WHEEL_UP_BUTTON, step.button)
        assertEquals(1, step.notches)
        assertEquals(0f, step.remainderPx, 0.01f)
    }

    @Test
    fun `sub line movement accumulates instead of emitting a notch per event`() {
        // 每事件 10px、行高 40px：四次事件才够一行，逐事件发一档就是四倍过冲。
        var remainder = 0f
        var notches = 0
        repeat(4) {
            val step = wheelDragStep(remainder, distanceY = -10f, cellHeightPx = 40f)
            notches += step.notches
            remainder = step.remainderPx
        }
        assertEquals(1, notches)
        assertEquals(0f, remainder, 0.01f)
    }

    @Test
    fun `remainder never grows unbounded across many small events`() {
        var remainder = 0f
        var notches = 0
        repeat(100) {
            val step = wheelDragStep(remainder, distanceY = -13f, cellHeightPx = 40f)
            notches += step.notches
            remainder = step.remainderPx
        }
        // 100 × 13px = 1300px = 32.5 行：必须发出 32 档，余半行。
        assertEquals(32, notches)
        assertEquals(-20f, remainder, 0.01f)
    }

    @Test
    fun `a single huge event is capped and keeps the excess as remainder`() {
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = -40_000f, cellHeightPx = 40f)
        assertEquals(MAX_WHEEL_NOTCHES_PER_EVENT, step.notches)
        assertEquals(WHEEL_DOWN_BUTTON, step.button)
        // 超出上限的位移不被丢弃：留作余量，由后续事件继续消费。
        assertTrue(step.remainderPx < 0f)
    }

    @Test
    fun `zero cell height still yields a usable step`() {
        // 度量尚未就绪时行高为 0：不得除零成无穷档，按最小行高换算。
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = -3f, cellHeightPx = 0f)
        assertEquals(WHEEL_DOWN_BUTTON, step.button)
        assertEquals(3, step.notches)
        assertEquals(0f, step.remainderPx, 0.01f)
    }
}
