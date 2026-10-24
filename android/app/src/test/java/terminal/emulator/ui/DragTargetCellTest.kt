package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.HandleDragResult
import terminal.emulator.SelectionAnchor
import terminal.emulator.SelectionState
import terminal.emulator.withPendingDrag

/**
 * 手柄拖动的触点折算。
 *
 * 外部真相：DESIGN.md「选区随滚动跟随，支持宽字符吸附 交叉交换归属」。
 * 交叉交换归属的含义是：被拖的手柄越过另一侧后，归属互换（被拖的柄变成另一端），
 * 而**手指落点仍是边界**——被拖的边界必须停在手指所在格，不能反向跳回。
 *
 * 参考点（首端柄 = 首格左缘，末端柄 = 末格右缘）在整次拖动内不变：
 * 参考点若随归属翻转而换，目标格会在翻转瞬间跳一格，边界反向移动，
 * 交叉也会被立刻撤销（选中范围塌成起点单格）。
 */
class DragTargetCellTest {

    private val cellWidth = 10f
    private val cellHeight = 20f
    private val cols = 40
    private val rows = 24

    /** 抓住末端柄（参考点 = 末格右缘）时的目标列。 */
    private fun endHandleTargetColumn(anchorCol: Int, touchX: Float): Int = dragTargetColumn(
        anchorCol = anchorCol,
        anchorIsStart = false,
        touchX = touchX,
        cellWidth = cellWidth,
        cols = cols,
    )

    /** 抓住首端柄（参考点 = 首格左缘）时的目标列。 */
    private fun startHandleTargetColumn(anchorCol: Int, touchX: Float): Int = dragTargetColumn(
        anchorCol = anchorCol,
        anchorIsStart = true,
        touchX = touchX,
        cellWidth = cellWidth,
        cols = cols,
    )

    // ── 参考点 ────────────────────────────────────────────────────────────

    @Test
    fun `touch at the grabbed edge resolves to the grabbed cell`() {
        // 末端柄锚在第 10 格右缘（x = 110），手指按在该处 → 目标仍是第 10 格。
        assertEquals(10, endHandleTargetColumn(10, 11 * cellWidth))
        // 首端柄锚在第 2 格左缘（x = 20），手指按在该处 → 目标仍是第 2 格。
        assertEquals(2, startHandleTargetColumn(2, 2 * cellWidth))
    }

    @Test
    fun `one cell of finger movement moves the target exactly one cell`() {
        assertEquals(11, endHandleTargetColumn(10, 11 * cellWidth + cellWidth))
        assertEquals(9, endHandleTargetColumn(10, 11 * cellWidth - cellWidth))
        assertEquals(3, startHandleTargetColumn(2, 2 * cellWidth + cellWidth))
    }

    @Test
    fun `target column is clamped to the grid`() {
        assertEquals(0, endHandleTargetColumn(2, -100f))
        assertEquals(cols - 1, endHandleTargetColumn(cols - 2, 10_000f))
    }

    // ── 交叉翻转 ──────────────────────────────────────────────────────────

    @Test
    fun `crossing flips ownership and the dragged boundary stays on the finger cell`() {
        // 选区 [2, 10]，抓住末端柄（参考点 x = 110）向左拖过首端柄。
        val committed =
            SelectionState(
                active = true,
                dragging = true,
                start = SelectionAnchor(0, 2),
                end = SelectionAnchor(0, 10),
            )
        var pending: HandleDragResult? = null
        var draggingStart = false
        // 手指逐格左移，最后停在第 0 格右缘（x = 10，末端柄语义下即第 0 格）。
        for (touchX in listOf(110f, 90f, 70f, 50f, 30f, 20f, 10f)) {
            val targetColumn = endHandleTargetColumn(10, touchX)
            val result =
                committed.withPendingDrag(pending).applyHandleDrag(
                    draggingStart = draggingStart,
                    targetRow = 0,
                    targetCol = targetColumn,
                )
            pending = result
            draggingStart = result.draggingStart
        }
        val crossed = requireNotNull(pending)
        assertTrue("越过首端柄后归属必须翻转", crossed.draggingStart)
        assertEquals("被拖的边界必须停在手指所在格", 0, crossed.startCol)
        assertEquals("未拖动的一侧必须钉在原首端柄", 2, crossed.endCol)
    }

    @Test
    fun `crossing keeps the untouched boundary pinned at the pre-cross position`() {
        // 抓住首端柄（参考点 x = 20）向右拖过末端柄：未拖动的末端柄停在原位。
        val committed =
            SelectionState(
                active = true,
                dragging = true,
                start = SelectionAnchor(0, 2),
                end = SelectionAnchor(0, 10),
            )
        var pending: HandleDragResult? = null
        var draggingStart = true
        for (touchX in listOf(20f, 40f, 60f, 80f, 100f, 120f, 130f)) {
            val targetColumn = startHandleTargetColumn(2, touchX)
            val result =
                committed.withPendingDrag(pending).applyHandleDrag(
                    draggingStart = draggingStart,
                    targetRow = 0,
                    targetCol = targetColumn,
                )
            pending = result
            draggingStart = result.draggingStart
        }
        val crossed = requireNotNull(pending)
        assertTrue("越过末端柄后归属必须翻转", !crossed.draggingStart)
        assertEquals("未拖动的一侧必须钉在原末端柄", 10, crossed.startCol)
        assertEquals("被拖的边界必须停在手指所在格", 13, crossed.endCol)
    }

    // ── 行 ────────────────────────────────────────────────────────────────

    @Test
    fun `row follows the finger and is clamped to the viewport`() {
        // 手柄悬在格底：锚点行 5 的底缘 y = 120，手指压在该行 → 仍是第 5 行。
        assertEquals(5, dragTargetRow(5, 6 * cellHeight, cellHeight, rows))
        // 手指上移两行 → 第 3 行。
        assertEquals(3, dragTargetRow(5, 4 * cellHeight, cellHeight, rows))
        // 越出视口上下缘时钳位。
        assertEquals(0, dragTargetRow(5, -100f, cellHeight, rows))
        assertEquals(rows - 1, dragTargetRow(5, 10_000f, cellHeight, rows))
    }
}
