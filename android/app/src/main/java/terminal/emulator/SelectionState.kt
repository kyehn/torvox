package terminal.emulator

import terminal.emulator.ui.SelectionBounds
import terminal.emulator.ui.normalizeSelectionAnchors

/** 上游选择派生查询回传的界限数组长度：[startRow, startCol, endRow, endCol]。 */
internal const val SELECTION_BOUNDS_LENGTH = 4

data class SelectionAnchor(val row: Int, val col: Int)

data class SelectionState(
    val active: Boolean = false,
    val dragging: Boolean = false,
    val start: SelectionAnchor? = null,
    val end: SelectionAnchor? = null,
    val selectedText: String = "",
    /**
     * 仅粘贴选区：长按落在空白单元格（空行、空白格、行尾之后的列）。
     *
     * 这几类在 termux 的 `getSelectedText(x,y,x,y)` 下同归一档，故只有一个布尔
     * 取值；其余一切长按（有内容）都是普通文本选区。
     */
    val pasteOnly: Boolean = false,
    // 已执行菜单动作（复制/全选/分享）时置位：浮动菜单关闭而选区高亮保留。
    // 下次长按或拖动手柄移动时重置。
    val menuDismissed: Boolean = false,
) {
    val hasSelection: Boolean
        get() = active && start != null && end != null

    fun applyHandleDrag(draggingStart: Boolean, targetRow: Int, targetCol: Int): HandleDragResult {
        val currentEnd = end ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol, draggingStart)
        val currentStart = start ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol, draggingStart)
        // 严格不等：落点恰为锚点时不翻转（对标 termlib applyHandleDrag），等值翻转只会让
        // 下一帧按错柄解释，连续回拖时语义跳变。
        if (
            draggingStart &&
            (
                targetRow > currentEnd.row ||
                    (targetRow == currentEnd.row && targetCol > currentEnd.col)
                )
        ) {
            return HandleDragResult(currentEnd.row, currentEnd.col, targetRow, targetCol, false)
        }
        if (
            !draggingStart &&
            (
                targetRow < currentStart.row ||
                    (targetRow == currentStart.row && targetCol < currentStart.col)
                )
        ) {
            return HandleDragResult(targetRow, targetCol, currentStart.row, currentStart.col, true)
        }
        if (draggingStart) {
            return HandleDragResult(targetRow, targetCol, currentEnd.row, currentEnd.col, true)
        }
        return HandleDragResult(currentStart.row, currentStart.col, targetRow, targetCol, false)
    }
}

data class HandleDragResult(
    val startRow: Int,
    val startCol: Int,
    val endRow: Int,
    val endCol: Int,
    val draggingStart: Boolean,
)

/**
 * 快速拖动路径的**当前边界**：把上一次未提交的拖动结果叠加到已提交选区上。
 *
 * 快速路径刻意不写 `_state`（每帧一次 Compose 往返正是瓶颈），于是 `applyHandleDrag`
 * 看到的 `start`/`end` 仍停在拖动开始时的位置。越过另一侧手柄后归属已经翻转，
 * 下一帧若仍拿旧端点当锚点，就会把**没被拖动的**那一侧算到错误位置——现象正是
 * 「只动一侧，另一侧的匹配区域却变了」。故每次计算都必须以上一次的实时边界为基准。
 */
internal fun SelectionState.withPendingDrag(pending: HandleDragResult?): SelectionState =
    if (!active || pending == null) {
        this
    } else {
        copy(
            start = SelectionAnchor(pending.startRow, pending.startCol),
            end = SelectionAnchor(pending.endRow, pending.endCol),
            dragging = true,
        )
    }

/**
 * 一帧拖动的结果：以 [pending]（上一次未提交的实时边界）为基准折叠出新边界并钳位。
 *
 * 快速路径与慢速路径共用本函数。两侧各写一份时，任一侧漏掉 pending 就会在两条路径
 * 交替的瞬间（快速拖动 ←→ 边缘滚动）把**未拖动**的一侧弹回已提交的旧端点：
 * 现象正是「另一边界异常增加，手柄与边界不再对齐」。
 *
 * @param maxRow 绝对网格行上界（含）。
 * @param maxCol 列上界（含）。
 */
internal fun resolveDragBounds(
    current: SelectionState,
    pending: HandleDragResult?,
    draggingStart: Boolean,
    row: Int,
    col: Int,
    maxRow: Int,
    maxCol: Int,
): HandleDragResult {
    val result = current.withPendingDrag(pending).applyHandleDrag(draggingStart, row, col)
    val bounds = clampSelection(result.startRow, result.startCol, result.endRow, result.endCol, maxRow, maxCol)
    return HandleDragResult(
        bounds.startRow,
        bounds.startCol,
        bounds.endRow,
        bounds.endCol,
        result.draggingStart,
    )
}

/**
 * 长按拖动（没有独立手柄）的边界折叠。
 *
 * 调用方没有「正在拖哪个柄」这一信息：越过另一侧后归属已由 [SelectionState.applyHandleDrag]
 * 翻转到实时边界里，下一帧必须沿用它。若固定按 END 解释，反向回拖时
 * **未被拖动**的一侧会跟着手指走——用户看到的是「另一边界异常增加」。
 */
internal fun resolveLongPressDragBounds(
    current: SelectionState,
    pending: HandleDragResult?,
    row: Int,
    col: Int,
    maxRow: Int,
    maxCol: Int,
): HandleDragResult = resolveDragBounds(
    current = current,
    pending = pending,
    draggingStart = pending?.draggingStart ?: false,
    row = row,
    col = col,
    maxRow = maxRow,
    maxCol = maxCol,
)

/**
 * 选区端点对（行主序有序）：高亮下发与取文共用的唯一端点来源。
 *
 * 端点就是选区自身的两个锚点，**不是**行列各自取极值的包围盒角点。
 * 原生按**线性**语义解释端点（`SELECTION_RECTANGLE = false`）：跨行时首行自起始列
 * 至行尾、末行自行首至结束列。角点会在起始列大于结束列时把高亮区域在首行左侧与
 * 末行右侧同时外扩，而控制柄仍停在锚点上——现象正是「另一边界异常增加且两个指针
 * 和边界不再对齐」，且复制出的文本与高亮区域不是同一块。
 */
internal fun selectionEndpoints(start: SelectionAnchor, end: SelectionAnchor): SelectionBounds {
    val (first, second) = normalizeSelectionAnchors(start, end, { it.row }, { it.col })
    return SelectionBounds(first.row, first.col, second.row, second.col)
}
