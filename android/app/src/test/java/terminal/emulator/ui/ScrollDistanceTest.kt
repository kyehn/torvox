package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/** 手指滚动增量换算单元测试：方向、亚行累积、边缘钳制（无撕裂空白）。 */
class ScrollDistanceTest {

    private companion object {
        const val CELL_HEIGHT = 60f
        const val SCROLLBACK_LENGTH = 100
        const val MIDDLE_OFFSET = 10
    }

    @Test
    fun `finger down enters older history`() {
        // 下移为负：偏移增加，进入更早历史。
        val step = applyScrollDistance(0f, -60f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(11, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
    }

    @Test
    fun `finger up returns to newer content`() {
        // 上移为正：偏移减少，回到更新内容。
        val step = applyScrollDistance(0f, 60f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(9, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
    }

    @Test
    fun `sub cell movement accumulates without dropping`() {
        // 不足一行不跳变但保留余量，下次累积后整行移动（慢速不卡顿）。
        val first = applyScrollDistance(0f, -20f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET, first.newOffset)
        assertEquals(20f, first.newAccumulatorPx)
        val second =
            applyScrollDistance(first.newAccumulatorPx, -40f, CELL_HEIGHT, first.newOffset, SCROLLBACK_LENGTH)
        assertEquals(11, second.newOffset)
        assertEquals(0f, second.newAccumulatorPx)
    }

    @Test
    fun `full row steps are symmetric in both directions`() {
        val down = applyScrollDistance(0f, -120f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val up = applyScrollDistance(0f, 120f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(12, down.newOffset)
        assertEquals(8, up.newOffset)
    }

    @Test
    fun `top edge clamps and clears remainder`() {
        // 顶部继续上移：钳制为 0 并清负余量，避免空白累积。
        val step = applyScrollDistance(0f, 120f, CELL_HEIGHT, 0, SCROLLBACK_LENGTH)
        assertEquals(0, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
    }

    @Test
    fun `bottom edge clamps and clears remainder`() {
        // 底部继续下移：钳制为最大值并清正余量，避免空白累积。
        val step =
            applyScrollDistance(0f, -120f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH)
        assertEquals(SCROLLBACK_LENGTH, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
    }

    @Test
    fun `empty scrollback stays at bottom`() {
        val step = applyScrollDistance(0f, -60f, CELL_HEIGHT, 0, 0)
        assertEquals(0, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
    }

    @Test
    fun `degenerate cell height still scrolls without crash`() {
        val step = applyScrollDistance(0f, -10f, 0f, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET + 10, step.newOffset)
    }

    @Test
    fun `fling down keeps drag direction into older history`() {
        // 下移为正：惯性行速度为正，与拖动下移（偏移增加）同向。
        assertEquals(10, flingRowsPerSecond(600f, CELL_HEIGHT))
    }

    @Test
    fun `fling up keeps drag direction into newer content`() {
        // 上移为负：惯性行速度为负，与拖动上移（偏移减少）同向。
        assertEquals(-10, flingRowsPerSecond(-600f, CELL_HEIGHT))
    }

    @Test
    fun `drag and fling share the same direction sign`() {
        // 同一物理方向：拖动增量符号与惯性速度符号一致，锁定方向错误回归。
        val dragDown = applyScrollDistance(0f, -60f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val flingDown = flingRowsPerSecond(600f, CELL_HEIGHT)
        assert(dragDown.newOffset > MIDDLE_OFFSET && flingDown > 0)
        val dragUp = applyScrollDistance(0f, 60f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val flingUp = flingRowsPerSecond(-600f, CELL_HEIGHT)
        assert(dragUp.newOffset < MIDDLE_OFFSET && flingUp < 0)
    }
}
