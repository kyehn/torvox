package terminal.emulator.ui

import android.os.SystemClock
import terminal.emulator.util.runCatchingCancellable

/** 边缘滚动单步方向与行数：+1 向上（回滚多露一行）、-1 向下（少露一行）。 */
private const val EDGE_SCROLL_STEP_UP = 1
private const val EDGE_SCROLL_STEP_DOWN = -1

/**
 * 拥有选区手柄拖动：拖动状态、锚点、边缘滚动与宽字符吸附。
 *
 * 手柄覆盖层（[SelectionHandles]）与终端 Surface 的触摸路径共用本类：
 * 前者转发拖动事件，后者在移动帧里读锁定指针。拖动锚点语义对标 termlib 的
 * applyHandleDrag（交叉翻转由 `SelectionState.applyHandleDrag` 实现）。
 */
internal class HandleDragController(private val surface: TerminalSurface) {
    var state: HandleDrag = HandleDrag.NONE

    /**
     * 指针 id 锁定（多指针漂移）：在 ACTION_DOWN 时由锁定拖动的那根手指设定
     * （手柄命中测试或手柄弹窗）；后续 ACTION_MOVE 只取该指针在事件中的槽位，
     * 该指针已不在事件里（抬起或被回收）时整帧吞掉。UP/CANCEL 时清除。
     */
    var pointerId: Int? = null

    /** 最后一次拖动结束的 uptimeMillis；驱动 `shouldSuppressTapAfterDragEnd` 的菜单重显保护。 */
    var lastDragEndUptimeMs = 0L

    // 拖动锚点：拖动开始时被抓住手柄所固定的单元格边界（绝对网格坐标，0 = 回滚顶部）。
    // 拖动增量相对该锚点计算，因为手柄窗口悬于其锚定单元格之下
    // ——直接用触摸像素会解析到边界下方的那一行。
    // 锚点语义对标 termlib 的 applyHandleDrag（Terminal.kt:2294 起）；
    // 交叉翻转由 [terminal.emulator.SelectionState.applyHandleDrag] 实现并经
    // SelectionStateTest 覆盖（拖动手柄越过静止手柄时归属权交换，调用方依返回更新被拖柄），
    // 此处只负责把触点换算成网格目标。
    private var anchorRow = 0
    private var anchorCol = 0

    // 抓柄时握住的是哪一侧：决定触点列的参考点（首格左缘 / 末格右缘）。
    // 整次拖动内不变——手柄越过另一侧时只换归属；参考点跟着换会让目标列跳一格。
    private var anchorIsStart = false

    /** 拖动开始时快照的备用屏标志（MOVE 帧绝不做 JNI 调用）。 */
    private var altScreenSnapshot = false

    /** 最近一次 MOVE 的触点位置（边缘滚动步进时按它折算列）。 */
    private var currentTouchX = 0f
    private var currentTouchY = 0f

    /** 拖动会话内宽字符尾格缓存的行号与列集合。 */
    private var cachedWideCharTailRow = -1
    private var cachedWideCharTailCols: IntArray = IntArray(0)

    /**
     * 拖动会话进行中：期间只按行号复用缓存。
     *
     * 抬起与开新拖动都必须让缓存失效——行内容在这两次之间可能被新输出改写，
     * 而宽字符吸附依赖的正是该行当前的尾格列。只清标志而保留缓存时，下一次拖动
     * 在同一行号上直接命中上一次拖动的尾格，吸附到早已不存在的边界。
     */
    private var wideCharDragSession = false

    internal fun latchAnchor(which: HandleDrag, pointerId: Int? = null) {
        state = which
        // 抓柄即隐藏（design 决策 3）：拖动中菜单不遮挡选择；抬手由
        // finish 按新几何重锚重显。
        surface.hideSelectionMenu("latchDragAnchor")
        // 锁定到发起拖动的那根手指：来自其他指针的后续 MOVE 事件
        // 绝不能改变选区。
        this.pointerId = pointerId
        // 按拖动快照化状态，使 MOVE 帧发出零次 JNI 调用。
        altScreenSnapshot =
            runCatchingCancellable { surface.viewModel?.runtime?.bridge()?.isAltScreenActive() ?: false }
                .getOrDefault(false)
        invalidateWideCharTailCache()
        wideCharDragSession = true
        // 在拖动开始时只调用一次 setSelectionDragging(true)，
        // 使渲染线程抑制新输出引起的滚动复位。原先在 dragSelection 内
        // 按每个 MOVE 调用——鉴于它是幂等的，那纯属浪费。
        surface.viewModel?.runtime?.setSelectionDragging(true)
        // 抓住仅粘贴选区的手柄会把它升级为文本选区，
        // 使拖动能扩展范围，而不被「仅粘贴不可变」守卫吞掉。
        surface.viewModel?.beginHandleDragOnPasteOnly()
        val selection = surface.viewModel?.state?.value?.selection
        anchorIsStart = which == HandleDrag.START
        if (which == HandleDrag.START) {
            anchorRow = selection?.start?.row ?: 0
            anchorCol = selection?.start?.col ?: 0
        } else {
            anchorRow = selection?.end?.row ?: 0
            anchorCol = selection?.end?.col ?: 0
        }
    }

    /**
     * 手柄拖动的 ACTION_MOVE 主体，从 onTouchEvent 抽出（detekt NestedBlockDepth）：
     * 指针锁定的选区更新 + 在视口 [touchX]/[touchY] 处的边缘滚动步进
     * （调用方已把坐标解析到锁定手指的槽位）。
     */
    internal fun driveMove(touchX: Float, touchY: Float) {
        currentTouchX = touchX
        currentTouchY = touchY

        // 备用屏（TUI）：禁用边缘滚动——该拖动属于远端全屏缓冲，
        // 而非本地回滚（termux TextSelectionCursorController :218-337 语义）。优先级：
        // SCROLL 锁 > 备用屏禁用 > 普通滚动。
        // 使用拖动开始时的快照：MOVE 帧发出零次 JNI 调用。
        val altScreenActive = altScreenSnapshot
        when (edgeScrollDirection(touchY, surface.surfaceHeightPixels.toFloat(), surface.cellHeight)) {
            EdgeScrollDirection.UP -> {
                // 每次触点移动滚 1 行（design 决策 5）：无定时循环，滚动只随手指移动发生。
                if (!altScreenActive) {
                    stepEdgeScroll(EDGE_SCROLL_STEP_UP)
                }
            }

            EdgeScrollDirection.DOWN -> {
                if (!altScreenActive) {
                    stepEdgeScroll(EDGE_SCROLL_STEP_DOWN)
                }
            }

            EdgeScrollDirection.STOP -> {
                val (gridRow, snapCol) = dragTargetFromTouch(touchX, touchY)
                // 快速拖动路径：直接计算边界并重定位手柄，
                // 避开 Compose _state.update → 重组 → 回读的往返
                // ——那正是每次 MOVE 的主要帧瓶颈。
                val result =
                    surface.viewModel?.dragMove(
                        draggingStart = state == HandleDrag.START,
                        row = gridRow,
                        col = snapCol,
                        cachedMaxRow = (surface.currentScrollbackLength() + surface.rows - 1).coerceAtLeast(0),
                        cachedMaxCol = (surface.cols - 1).coerceAtLeast(0),
                    )
                if (result != null) {
                    state = if (result.draggingStart) HandleDrag.START else HandleDrag.END
                    surface.selectionHandles.repositionHandle(HandleDrag.START, result.startRow, result.startCol)
                    surface.selectionHandles.repositionHandle(HandleDrag.END, result.endRow, result.endCol)
                }
            }
        }
    }

    /**
     * 手柄拖动结束的共用提交——由覆盖层所属的拖动路径
     * （[SelectionHandles.HandleOverlayLayout]）与旧的 Surface 触摸路径共同调用：
     * 把最终选区提交到 Compose 状态、启用 300ms 轻击保护（termux 隐藏保护）、
     * 重新显示已吸附到最终单元格的手柄与工具栏，
     * 并把高亮刷新到 Rust 渲染器。
     */
    internal fun finish() {
        state = HandleDrag.NONE
        pointerId = null
        invalidateWideCharTailCache()
        surface.viewModel?.commitDragBounds()
        surface.viewModel?.endSelection()
        lastDragEndUptimeMs = SystemClock.uptimeMillis()
        surface.reshowSelectionHandles()
        surface.showSelectionMenuForCurrentSelection()
        surface.viewModel?.runtime?.forceRender()
    }

    /** 拖动结束（未走 [finish] 的路径，如长按拖动）也要记一笔，供轻击保护使用。 */
    internal fun markDragEnded() {
        lastDragEndUptimeMs = SystemClock.uptimeMillis()
    }

    /** 复位拖动状态：手势收尾与 Surface 拆除都调它。 */
    internal fun reset() {
        state = HandleDrag.NONE
        pointerId = null
        invalidateWideCharTailCache()
    }

    private fun dragTargetFromTouch(touchX: Float, touchY: Float): Pair<Int, Int> {
        val viewportTopGrid = surface.currentViewportTopGrid()
        val viewportRow =
            dragTargetRow(
                anchorViewportRow = anchorRow - viewportTopGrid,
                touchY = touchY,
                cellHeight = surface.cellHeight,
                rows = surface.rows,
            )
        val col =
            dragTargetColumn(
                anchorCol = anchorCol,
                anchorIsStart = anchorIsStart,
                touchX = touchX,
                cellWidth = surface.cellWidth,
                cols = surface.cols,
            )
        val gridRow = viewportTopGrid + viewportRow
        return gridRow to snapToWideCharBoundary(gridRow, col)
    }

    /**
     * 边缘滚动单步：触点位于边缘区的每次 MOVE 滚 [step]（[EDGE_SCROLL_STEP_UP] 向上、
     * [EDGE_SCROLL_STEP_DOWN] 向下）恰好 1 行，滚动随手指移动即时发生（无定时循环），
     * 并把拖拽柄钉到新视口的顶/底行后同步两个手柄位置。
     */
    private fun stepEdgeScroll(step: Int) {
        val scrollbackLen = surface.rebaseScrollOffset()
        val newOffset = (surface.scrollOffset + step).coerceIn(0, scrollbackLen)
        if (newOffset != surface.scrollOffset) {
            surface.applyScrollOffset(newOffset)
            // 向上滚钉新视口顶行，向下滚钉底行。
            val gridRow =
                if (step == EDGE_SCROLL_STEP_UP) {
                    scrollbackLen - newOffset
                } else {
                    scrollbackLen - newOffset + surface.rows - 1
                }
            updateDragHandleForCell(gridRow)
        }
    }

    /**
     * 把被拖动的手柄移到当前触摸列所在 [gridRow] 的单元格，并在宽字符（CJK）边界处吸附。
     *
     * 与快速路径走同一个入口（`TerminalViewModel.dragMove`）：两条路径各自维护边界时，
     * 一条写入的端点会被另一条读到的旧端点覆盖——越过对侧手柄后尤其明显，
     * 未拖动的一侧会异常增长、手柄与选区边界对不上。
     */
    private fun updateDragHandleForCell(gridRow: Int) {
        // 列沿用与快速路径同一套折算（锚点 + 手指位移）：边缘滚动区若改用
        // 「手指所在格」，手指进出边缘区时列会跳一格。
        val curCol =
            dragTargetColumn(
                anchorCol = anchorCol,
                anchorIsStart = anchorIsStart,
                touchX = currentTouchX,
                cellWidth = surface.cellWidth,
                cols = surface.cols,
            )
        val snappedCol = snapToWideCharBoundary(gridRow, curCol)
        val result =
            surface.viewModel?.dragMove(
                draggingStart = state == HandleDrag.START,
                row = gridRow,
                col = snappedCol,
                cachedMaxRow = (surface.currentScrollbackLength() + surface.rows - 1).coerceAtLeast(0),
                cachedMaxCol = (surface.cols - 1).coerceAtLeast(0),
            ) ?: return
        state = if (result.draggingStart) HandleDrag.START else HandleDrag.END
        surface.selectionHandles.repositionHandle(HandleDrag.START, result.startRow, result.startCol)
        surface.selectionHandles.repositionHandle(HandleDrag.END, result.endRow, result.endCol)
    }

    /**
     * 把列吸附到宽字符边界：当 `col` 落在宽字符的后半部分时回退一格，
     * 使选区手柄绝不把一个宽字符从中间切开。
     *
     * 判定取自网格单元宽度（原生 [terminal.emulator.bridge.Bridge.wideCharTailCols]）：
     * 每列恰好一个字符，宽字符尾格在该字符串里是空格占位，与真空白不可区分——
     * 按文本反推必然出错。
     *
     * 整行的尾格列在一次拖动会话内只查一次：吸附在每个 MOVE 帧都要做，而该查询是
     * 发往 VT 线程的同步往返（空闲时最坏约一个 50ms 查询节拍）。行只经边缘滚动变化，
     * 而那本身会换行号并重新取数。
     */
    internal fun snapToWideCharBoundary(gridRow: Int, col: Int): Int {
        if (col <= 0) return col
        val tailCols = wideCharTailCols(gridRow)
        return if (tailCols.contains(col)) col - 1 else col
    }

    /** 拖动会话内 [snapToWideCharBoundary] 的单项缓存：一次取整行，逐帧复用。 */
    private fun wideCharTailCols(gridRow: Int): IntArray {
        val bridge = surface.viewModel?.runtime?.bridge() ?: return IntArray(0)
        if (wideCharDragSession && gridRow == cachedWideCharTailRow) return cachedWideCharTailCols
        val tailCols = bridge.wideCharTailCols(gridRow)
        cachedWideCharTailRow = gridRow
        cachedWideCharTailCols = tailCols
        return tailCols
    }

    private fun invalidateWideCharTailCache() {
        cachedWideCharTailRow = -1
        cachedWideCharTailCols = IntArray(0)
        wideCharDragSession = false
    }
}
