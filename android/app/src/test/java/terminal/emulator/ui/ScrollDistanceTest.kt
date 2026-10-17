package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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
    fun `fling velocity is kept in pixels per second`() {
        // 除以行高换成行/秒会让逐帧落点被截成整行，内容只能逐行跳（STATUS ⑩）：
        // 600px/s 在 60px 行高下必须是 600，绝不能是 10。
        assertEquals(600, flingVelocityPxPerSecond(600f))
    }

    @Test
    fun `fling velocity keeps drag direction`() {
        // 下移为正（更旧）、上移为负（更新），与 applyScrollDistance 的偏移同向。
        assertTrue(flingVelocityPxPerSecond(600f) > 0)
        assertTrue(flingVelocityPxPerSecond(-600f) < 0)
    }

    @Test
    fun `drag and fling share the same direction sign`() {
        // 同一物理方向：拖动增量符号与惯性速度符号一致，锁定方向错误回归。
        val dragDown = applyScrollDistance(0f, -60f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val flingDown = flingVelocityPxPerSecond(600f)
        assertTrue(
            "下滑后内容必须上移且惯性方向向上（offset=${dragDown.newOffset} fling=$flingDown）",
            dragDown.newOffset > MIDDLE_OFFSET && flingDown > 0,
        )
        val dragUp = applyScrollDistance(0f, 60f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val flingUp = flingVelocityPxPerSecond(-600f)
        assertTrue(
            "上滑后内容必须下移且惯性方向向下（offset=${dragUp.newOffset} fling=$flingUp）",
            dragUp.newOffset < MIDDLE_OFFSET && flingUp < 0,
        )
    }

    @Test
    fun `slow fling advances by pixels without row quantization`() {
        // 600px/s ≈ 10px/帧：前五帧不足一行，偏移不动、余量按像素累积（渲染器据此
        // 行内跟随），第六帧满一行才进位。整行驱动的惯性在第一帧就会跳一整行。
        var accumulator = 0f
        var offset = MIDDLE_OFFSET
        repeat(5) {
            val step = applyScrollDistance(accumulator, -10f, CELL_HEIGHT, offset, SCROLLBACK_LENGTH)
            accumulator = step.newAccumulatorPx
            offset = step.newOffset
        }
        assertEquals(MIDDLE_OFFSET, offset)
        assertEquals(50f, accumulator)
        val sixth = applyScrollDistance(accumulator, -10f, CELL_HEIGHT, offset, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET + 1, sixth.newOffset)
        assertEquals(0f, sixth.newAccumulatorPx)
    }

    // ── R3: 截断趋向零对称性 ─────────────────────────────────────────────

    @Test
    fun `sub cell movement is symmetric across zero`() {
        // floor 非对称修复：+0.9px 和 -0.9px 均不触发行变（截断趋向零）。
        val up = applyScrollDistance(0f, 0.9f * CELL_HEIGHT, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val down = applyScrollDistance(0f, -0.9f * CELL_HEIGHT, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET, up.newOffset)
        assertEquals(MIDDLE_OFFSET, down.newOffset)
    }

    @Test
    fun `round trip of equal magnitude drag cancels out`() {
        // 往返等量拖动（1.5 行上 + 1.5 行下）应回到原位。
        val upStep = applyScrollDistance(0f, 1.5f * CELL_HEIGHT, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET - 1, upStep.newOffset)
        val downStep =
            applyScrollDistance(
                upStep.newAccumulatorPx,
                -1.5f * CELL_HEIGHT,
                CELL_HEIGHT,
                upStep.newOffset,
                SCROLLBACK_LENGTH,
            )
        assertEquals(MIDDLE_OFFSET, downStep.newOffset)
    }

    @Test
    fun `slightly over one cell triggers row change in both directions`() {
        // 超过 1 行余量时双向均触发行变。
        val up = applyScrollDistance(0f, 1.1f * CELL_HEIGHT, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        val down = applyScrollDistance(0f, -1.1f * CELL_HEIGHT, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET - 1, up.newOffset)
        assertEquals(MIDDLE_OFFSET + 1, down.newOffset)
    }

    @Test
    fun `zero distance does not change offset`() {
        val step = applyScrollDistance(0f, 0f, CELL_HEIGHT, MIDDLE_OFFSET, SCROLLBACK_LENGTH)
        assertEquals(MIDDLE_OFFSET, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
    }

    // 边界外位移 MUST 就地丢弃：下发成渲染器余量会让整幅网格被投影亚行平移、边缘露出
    // 背景空带，攒够一整行再清零又整体弹回——即「滑到边界跳动闪烁」。

    @Test
    fun `overscroll at the top edge is discarded instead of leaking to the renderer`() {
        val step = applyScrollDistance(0f, -20f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH)
        assertEquals(SCROLLBACK_LENGTH, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
        assertEquals(20f, step.overflowPx)
    }

    @Test
    fun `overscroll at the bottom edge is discarded symmetrically`() {
        val step = applyScrollDistance(0f, 20f, CELL_HEIGHT, 0, SCROLLBACK_LENGTH)
        assertEquals(0, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
        assertEquals(-20f, step.overflowPx)
    }

    @Test
    fun `leaving the top edge keeps the sub cell remainder`() {
        // 边界**内**的亚行位移是合法的行内跟随，不得被当成越界丢弃。
        val step = applyScrollDistance(0f, 20f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH)
        assertEquals(SCROLLBACK_LENGTH, step.newOffset)
        assertEquals(-20f, step.newAccumulatorPx)
        assertEquals(0f, step.overflowPx)
    }

    // 位移显出量：上滑到回滚区顶端后继续上滑，把被键盘位移裁到屏幕上边界之外的网格顶部
    // 按像素放回屏幕（STATUS「输入法弹出时上滑不能到顶部」）。

    @Test
    fun `overscroll past the top reveals the clipped grid top`() {
        val step = applyScrollDistance(0f, -20f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH, 0f, 130f)
        assertEquals(SCROLLBACK_LENGTH, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
        assertEquals(0f, step.overflowPx)
        assertEquals(20f, step.revealPx)
    }

    @Test
    fun `revealing stops at the shift capacity`() {
        val step = applyScrollDistance(0f, -40f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH, 130f, 130f)
        assertEquals(SCROLLBACK_LENGTH, step.newOffset)
        assertEquals(130f, step.revealPx)
        assertEquals(40f, step.overflowPx)
    }

    @Test
    fun `returning inside the top edge reclaims the reveal one to one`() {
        // 反向先 1:1 回收显出量：20px 手指位移只回 20px，不滚动、不出现 2 倍行程。
        val step = applyScrollDistance(0f, 20f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH, 130f, 130f)
        assertEquals(SCROLLBACK_LENGTH, step.newOffset)
        assertEquals(0f, step.newAccumulatorPx)
        assertEquals(110f, step.revealPx)
    }

    @Test
    fun `returning past the reveal scrolls only the excess`() {
        val step = applyScrollDistance(0f, 200f, CELL_HEIGHT, SCROLLBACK_LENGTH, SCROLLBACK_LENGTH, 130f, 130f)
        assertEquals(SCROLLBACK_LENGTH - 1, step.newOffset)
        assertEquals(-10f, step.newAccumulatorPx)
        assertEquals(0f, step.revealPx)
    }
}
