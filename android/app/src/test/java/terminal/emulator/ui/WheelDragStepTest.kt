package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 触摸拖动 → 滚轮档位的换算。
 *
 * 断言三个不变式：方向为「内容跟随手指」（手指下移 = 滚轮上，与本地回滚及 termux 同向）、
 * 刻度为「走满一行才发一档」（否则慢速拖动被放大成逐事件一档）、余量以同一符号约定累积
 * （正 = 更旧的待消化位移，与 [applyScrollDistance] 共用同一累加器）。
 */
class WheelDragStepTest {

    @Test
    fun `finger moving down forwards wheel up`() {
        // distanceY 为前一帧减当前帧：手指下移为负，进入更早的历史 = 滚轮上。
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = -40f, cellHeightPx = 40f)
        assertEquals(WHEEL_UP_BUTTON, step.button)
        assertEquals(1, step.notches)
        assertEquals(0f, step.remainderPx, 0.01f)
    }

    @Test
    fun `finger moving up forwards wheel down`() {
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = 40f, cellHeightPx = 40f)
        assertEquals(WHEEL_DOWN_BUTTON, step.button)
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
        // 100 × 13px = 1300px = 32.5 行：必须发出 32 档，余半行（正 = 更旧待消化）。
        assertEquals(32, notches)
        assertEquals(20f, remainder, 0.01f)
    }

    @Test
    fun `a single huge event is capped and keeps the excess as remainder`() {
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = -40_000f, cellHeightPx = 40f)
        assertEquals(MAX_WHEEL_NOTCHES_PER_EVENT, step.notches)
        assertEquals(WHEEL_UP_BUTTON, step.button)
        // 超出上限的位移不被丢弃：留作余量，由后续事件继续消费。
        assertTrue(step.remainderPx > 0f)
    }

    @Test
    fun `zero cell height still yields a usable step`() {
        // 度量尚未就绪时行高为 0：不得除零成无穷档，按最小行高换算。
        val step = wheelDragStep(accumulatorPx = 0f, distanceY = -3f, cellHeightPx = 0f)
        assertEquals(WHEEL_UP_BUTTON, step.button)
        assertEquals(3, step.notches)
        assertEquals(0f, step.remainderPx, 0.01f)
    }

    @Test
    fun `truncation toward zero is symmetric in both directions`() {
        // 不足一行时两向都不得成档：floor 会在负方向多发一档（-0.9 行 → -1 行）。
        val older = wheelDragStep(accumulatorPx = 0f, distanceY = -36f, cellHeightPx = 40f)
        val newer = wheelDragStep(accumulatorPx = 0f, distanceY = 36f, cellHeightPx = 40f)
        assertEquals(0, older.notches)
        assertEquals(0, newer.notches)
        assertEquals(36f, older.remainderPx, 0.01f)
        assertEquals(-36f, newer.remainderPx, 0.01f)
    }

    @Test
    fun `forwarded wheel direction matches the local scrollback for the same gesture`() {
        // 同一手势在两条路径上必须得到同一「往哪边看」的结论：转发路径发滚轮上时，
        // 本地路径也必须进入更早的历史。否则应用一开鼠标上报，手感就被反转。
        val cellHeight = 40f
        val olderWheel = wheelDragStep(0f, distanceY = -80f, cellHeightPx = cellHeight)
        val olderLocal = applyScrollDistance(0f, -80f, cellHeight, scrollOffset = 5, scrollbackLength = 20)
        assertEquals(WHEEL_UP_BUTTON, olderWheel.button)
        assertTrue("手指下移在本地路径同样进入更早的历史", olderLocal.newOffset > 5)

        val newerWheel = wheelDragStep(0f, distanceY = 80f, cellHeightPx = cellHeight)
        val newerLocal = applyScrollDistance(0f, 80f, cellHeight, scrollOffset = 5, scrollbackLength = 20)
        assertEquals(WHEEL_DOWN_BUTTON, newerWheel.button)
        assertTrue("手指上移在本地路径同样回到更新的内容", newerLocal.newOffset < 5)
    }
}
