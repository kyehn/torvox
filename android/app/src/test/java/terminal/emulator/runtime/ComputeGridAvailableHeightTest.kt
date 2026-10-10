package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 网格可用高度：Surface 高度扣掉修饰键栏覆盖层与输入法遮挡（备用屏）。
 *
 * 三条网格重算路径 MUST 共用同一算式。此前运行期那条漏了 `coerceAtLeast(1)`，
 * 「Surface 不高于键栏 + 输入法」的几何下它拿到 (0,0) 哨兵并**静默保持旧网格**
 * ——输入法扣减被整段丢掉；视图侧两条却收敛到一行。同一几何算出两种网格，
 * 正是 render-stability 要求的「扣减量单一来源」要防的分叉。
 */
class ComputeGridAvailableHeightTest {

    @Test
    fun `modifier bar and ime reserve are both subtracted`() {
        // 2400px Surface − 200px 键栏 = 2200；备用屏下再扣 1000px 键盘 → 1200。
        assertEquals(2200, computeGridAvailableHeight(2400, 200, 0))
        assertEquals(1200, computeGridAvailableHeight(2400, 200, 1000))
    }

    @Test
    fun `a larger bar and a smaller reserve give the same height`() {
        // 同一 1200px 结果的两种来源，证明两者是**相加**而非取较大者：
        // 2400 − 200 − 1000 与 2400 − 1200 − 0。取「较大者」会得 1400 或 2200，
        // 两种都让键盘少扣或根本没扣（具体数值见同文件上一条用例）。
        assertEquals(1200, computeGridAvailableHeight(2400, 1200, 0))
        assertEquals(1200, computeGridAvailableHeight(2400, 200, 1000))
    }

    @Test
    fun `zero reserve leaves the modifier bar as the only subtraction`() {
        assertEquals(2200, computeGridAvailableHeight(2400, 200, 0))
        assertEquals(2400, computeGridAvailableHeight(2400, 0, 0))
    }

    @Test
    fun `fully covered surface still yields one row instead of the degenerate sentinel`() {
        // 键盘 + 键栏盖满 Surface：不得返回 0/负数，否则调用方按哨兵静默保持旧网格。
        assertEquals(1, computeGridAvailableHeight(surfaceHeight = 2400, modifierBarHeightPx = 200, imeReserve = 2200))
        assertEquals(1, computeGridAvailableHeight(surfaceHeight = 200, modifierBarHeightPx = 200, imeReserve = 0))
        assertEquals(1, computeGridAvailableHeight(surfaceHeight = 2400, modifierBarHeightPx = 500, imeReserve = 3000))
    }

    @Test
    fun `the clamped height is what the grid formula consumes`() {
        // 端到端：覆盖满时行数是 1 而不是被调用方当成「几何无效」丢弃。
        val (rows, cols) =
            computeGridDimensions(
                surfaceWidth = 1080,
                surfaceHeight =
                computeGridAvailableHeight(
                    surfaceHeight = 2400,
                    modifierBarHeightPx = 200,
                    imeReserve = 2200,
                ),
                cellWidth = 30f,
                cellHeight = 60f,
            )
        assertEquals(1, rows)
        assertEquals(36, cols)
    }
}
