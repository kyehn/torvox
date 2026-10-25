package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure selection-state helpers: handle-drag crossing flips. No native
 * engine involved — the rules are exercised directly on [SelectionState].
 */
class SelectionStateTest {
    private fun selection(startRow: Int = 0, startCol: Int = 0, endRow: Int = 0, endCol: Int = 10) = SelectionState(
        active = true,
        start = SelectionAnchor(startRow, startCol),
        end = SelectionAnchor(endRow, endCol),
    )

    // ── applyHandleDrag crossing flip ────────────────────────────────────

    @Test
    fun `dragging start past end swaps ownership`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 12)
        // START swept past END: new start pins at the old END, END follows the finger.
        assertEquals(HandleDragResult(0, 10, 0, 12, false), result)
    }

    @Test
    fun `dragging start below end swaps ownership across rows`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 2, endCol = 5)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 3, targetCol = 0)
        assertEquals(HandleDragResult(2, 5, 3, 0, false), result)
    }

    @Test
    fun `dragging end before start swaps ownership`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 0)
        assertEquals(HandleDragResult(0, 0, 0, 2, true), result)
    }

    @Test
    fun `normal start drag keeps ordering`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 5)
        assertEquals(HandleDragResult(0, 5, 0, 10, true), result)
    }

    @Test
    fun `normal end drag keeps ordering`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val result = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 12)
        assertEquals(HandleDragResult(0, 2, 0, 12, false), result)
    }

    @Test
    fun `landing exactly on anchor does not flip`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val startLandsOnEnd = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 10)
        assertEquals(HandleDragResult(0, 10, 0, 10, true), startLandsOnEnd)
        val endLandsOnStart = s.applyHandleDrag(draggingStart = false, targetRow = 0, targetCol = 2)
        assertEquals(HandleDragResult(0, 2, 0, 2, false), endLandsOnStart)
    }

    @Test
    fun `cross then drag back follows flipped handle`() {
        val s = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val crossed = s.applyHandleDrag(draggingStart = true, targetRow = 0, targetCol = 12)
        assertEquals(false, crossed.draggingStart)
        val moved =
            selection(
                startRow = crossed.startRow,
                startCol = crossed.startCol,
                endRow = crossed.endRow,
                endCol = crossed.endCol,
            )
        val back = moved.applyHandleDrag(draggingStart = crossed.draggingStart, targetRow = 0, targetCol = 11)
        assertEquals(HandleDragResult(0, 10, 0, 11, false), back)
    }

    // ── withPendingDrag：快速路径以未提交的实时边界为基准 ──────────────

    @Test
    fun `pending drag keeps stationary handle pinned after crossing`() {
        // 拖动开始时的已提交选区仍停在 (0,2)-(0,10)；手指已把 START 拖到 12，
        // 归属翻转为 END。回拖到 11 时，未被拖动的手柄必须留在 10。
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val pending = HandleDragResult(0, 10, 0, 12, false)
        val next =
            committed.withPendingDrag(pending).applyHandleDrag(
                draggingStart = pending.draggingStart,
                targetRow = 0,
                targetCol = 11,
            )
        assertEquals(HandleDragResult(0, 10, 0, 11, false), next)
    }

    @Test
    fun `pending drag keeps stationary handle pinned after crossing left`() {
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val pending = HandleDragResult(0, 0, 0, 2, true)
        val next =
            committed.withPendingDrag(pending).applyHandleDrag(
                draggingStart = pending.draggingStart,
                targetRow = 0,
                targetCol = 1,
            )
        assertEquals(HandleDragResult(0, 1, 0, 2, true), next)
    }

    @Test
    fun `pending drag is ignored when absent or inactive`() {
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        assertEquals(committed, committed.withPendingDrag(null))
        val inactive = committed.copy(active = false)
        assertEquals(inactive, inactive.withPendingDrag(HandleDragResult(0, 0, 0, 1, true)))
    }

    // ── 快速路径与慢速路径共用同一份实时边界 ──────────────────────────

    @Test
    fun `slow path folds onto the pending boundary from the fast path`() {
        // 已提交选区仍是 (0,2)-(0,10)；快速路径已把 END 拖过 START 到 0，
        // 未提交的实时边界为 (0,0)-(0,2)、被拖柄翻成 START。
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val pending = HandleDragResult(0, 0, 0, 2, true)

        // 慢速路径（边缘滚动）下一帧把 START 拖到 1。
        val next =
            resolveDragBounds(
                current = committed,
                pending = pending,
                draggingStart = pending.draggingStart,
                row = 0,
                col = 1,
                maxRow = 23,
                maxCol = 79,
            )

        // 未被拖动的一侧必须钉在 2（交叉后的位置），不得弹回已提交的 10。
        assertEquals(HandleDragResult(0, 1, 0, 2, true), next)
    }

    @Test
    fun `fast path folds onto the boundary written by the slow path`() {
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        // 慢速路径刚把 END 拖到 16 并写入状态，实时边界即 (0,2)-(0,16)。
        val slow =
            resolveDragBounds(committed, null, draggingStart = false, row = 0, col = 16, maxRow = 23, maxCol = 79)
        assertEquals(HandleDragResult(0, 2, 0, 16, false), slow)
        // 快速路径接着把 END 拖到 20：基准是慢路径的 16，不是已提交的 10。
        val fast =
            resolveDragBounds(
                committed,
                slow,
                draggingStart = slow.draggingStart,
                row = 0,
                col = 20,
                maxRow = 23,
                maxCol = 79,
            )
        assertEquals(HandleDragResult(0, 2, 0, 20, false), fast)
    }

    @Test
    fun `drag bounds are clamped to the grid on both paths`() {
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 10)
        val bounds =
            resolveDragBounds(
                current = committed,
                pending = null,
                draggingStart = false,
                row = 99,
                col = 999,
                maxRow = 23,
                maxCol = 79,
            )
        assertEquals(HandleDragResult(0, 2, 23, 79, false), bounds)
    }

    @Test
    fun `long press reverse across the anchor keeps the far boundary pinned`() {
        // 长按拖动没有独立手柄：只能从实时边界推回归属。先从 (0,5) 拖到 1
        // 越过起点 2，再继续反向拖到 0，然后回拖过起点到 6、10。
        val committed = selection(startRow = 0, startCol = 2, endRow = 0, endCol = 5)
        var pending: HandleDragResult? = null
        fun move(col: Int): HandleDragResult =
            resolveLongPressDragBounds(committed, pending, row = 0, col = col, maxRow = 23, maxCol = 79)
                .also { pending = it }

        assertEquals(HandleDragResult(0, 1, 0, 2, true), move(1))
        // 关键帧：未被拖动的一侧必须钉在 2，旧实现会同时把它挪到 1。
        assertEquals(HandleDragResult(0, 0, 0, 2, true), move(0))
        assertEquals(HandleDragResult(0, 2, 0, 6, false), move(6))
        assertEquals(HandleDragResult(0, 2, 0, 10, false), move(10))
    }
}
