package terminal.emulator.runtime

import terminal.emulator.bridge.Bridge

// Surface 像素与单元格度量到网格行列的唯一换算处：引导占位网格、可用高度、
// 初始网格与切会话时的网格对齐都在此。
//
// 这些换算 MUST 共用同一套公式：网格重算有三条路径（运行期的
// `recomputeGridFromFontMetrics`、视图侧 `TerminalSurface.ResizeManager.applyGridResize`
// 与 `recomputeRowsColsImmediate`），各自展开算式时曾在同一几何下得到两种网格。
/**
 * 引导前的占位网格尺寸（xterm 的经典 24×80）。
 *
 * 单一来源：[RuntimeState] 的默认值、视图侧的初始镜像与 `buildConfig` 都取这里。
 * 三处各写一份时，改一处不会影响另两处——「首个会话的网格」与「视图在拿到真实
 * 尺寸前显示的网格」会分家，表现为启动瞬间的行列跳变。
 */
const val DEFAULT_GRID_ROWS = 24
const val DEFAULT_GRID_COLS = 80

/**
 * 启动尺寸是否可用。
 *
 * 只拒绝非正尺寸：分屏、自由窗口与折叠半屏都是合法的小窗口。此前按整块显示尺寸
 * 推导像素下界（宽度按等宽字形宽高比折算、高度按行高折半），小窗口一律被判成
 * 「等正确的 surfaceChanged」——会话建不出来，界面停在空白。
 *
 * 网格退化的情形由网格计算自身兜住：行列至少一行一列（见 [computeGridDimensions]）。
 */
internal fun isStartSizeUsable(width: Int, height: Int): Boolean = width > 0 && height > 0

/**
 * recomputeGridFromFontMetrics 背后的纯网格尺寸计算：cols = floor(surfaceWidth / cellWidth)，
 * rows = floor(surfaceHeight / cellHeight)，各自钳位到 ≥ 1，使极小 Surface 仍得到可用网格。
 * 退化输入（Surface 或单元格度量非正）返回 (0, 0)，
 * 使调用方能区分「几何无效」与真实的一行一列网格，并改为保持当前尺寸。
 */
internal fun computeGridDimensions(
    surfaceWidth: Int,
    surfaceHeight: Int,
    cellWidth: Float,
    cellHeight: Float,
): Pair<Int, Int> {
    if (surfaceWidth <= 0 || surfaceHeight <= 0 || cellWidth <= 0f || cellHeight <= 0f) {
        return Pair(0, 0)
    }
    val cols = (surfaceWidth / cellWidth).toInt().coerceAtLeast(1)
    val rows = (surfaceHeight / cellHeight).toInt().coerceAtLeast(1)
    return Pair(rows, cols)
}

/**
 * 网格的可用高度：Surface 高度扣掉修饰键栏覆盖层与输入法遮挡。
 *
 * 三条网格重算路径（运行期的 `recomputeGridFromFontMetrics`、
 * `TerminalSurface.ResizeManager.applyGridResize` 与 `recomputeRowsColsImmediate`）
 * MUST 共用本函数：它们此前各自展开这行算式，其中运行期那条还漏了 `coerceAtLeast`，
 * 于是「Surface 不高于键栏 + 输入法」这一几何下它返回 (0,0) 并**静默保持旧网格**
 * （输入法扣减被丢掉），另两条却收敛到一行——同一几何两种网格，正是
 * 「扣减量单一来源」要防的分叉。
 *
 * @param imeReserve [TerminalRuntime.imeGridReserve]：只在备用屏非零，主屏恒 0。
 */
internal fun computeGridAvailableHeight(surfaceHeight: Int, modifierBarHeightPx: Int, imeReserve: Int): Int =
    (surfaceHeight - modifierBarHeightPx - imeReserve).coerceAtLeast(1)

/**
 * spawn PTY 之前能确定的最佳网格：用原生字体度量与已知的 surface 像素尺寸算出真实
 * 行列，度量或尺寸尚未就绪时退回 [DEFAULT_GRID_ROWS]×[DEFAULT_GRID_COLS]（旧行为）。
 *
 * 为什么必须在 spawn 之前算：PTY 的 winsize 决定 shell 怎么排版它的第一个 prompt。
 * 先按默认 24×80 建 PTY、attach 之后才 resize，会让 shell 收到 SIGWINCH 重排**已经
 * 打印过**的 prompt——真机日志里新会话首屏正是从中间截断、第二行才补齐。
 * 用真实网格建 PTY 后，attach 之后的首次 resize 变成零变更（见
 * [TerminalRuntime.recomputeGridFromFontMetrics] 的相等早退）。
 */
internal fun initialSpawnGrid(
    surfaceWidth: Int,
    surfaceHeight: Int,
    cellWidth: Float,
    cellHeight: Float,
    modifierBarHeightPx: Int,
    imeReserve: Int,
): Pair<Int, Int> {
    // 守卫必须看 spawn 时的原始像素尺寸，不能只看 computeGridDimensions 的 (0,0) 哨兵：
    // computeGridAvailableHeight 会把「高度 ≤ 键栏 + 输入法」钳到 1 px，于是 0 高度的
    // surface 会绕过哨兵、被 computeGridDimensions 钳成一行——一个 1 行的 PTY 比默认网格更糟
    // （布局尚未发生时的尺寸竞态正是这样）。
    if (surfaceWidth <= 0 || surfaceHeight <= 0 || cellWidth <= 0f || cellHeight <= 0f) {
        return Pair(DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS)
    }
    return computeGridDimensions(
        surfaceWidth = surfaceWidth,
        surfaceHeight =
        computeGridAvailableHeight(
            surfaceHeight = surfaceHeight,
            modifierBarHeightPx = modifierBarHeightPx,
            imeReserve = imeReserve,
        ),
        cellWidth = cellWidth,
        cellHeight = cellHeight,
    )
}

/**
 * 切会话时是否需要对齐网格：目标会话原生网格与 UI 网格一致则跳过 `resize`
 * （无冗余 SIGWINCH）；查不到（0/异常）则返回 true 沿旧路无条件对齐。
 *
 * 网格查询以 lambda 传入：纯判定可单测，无需伪造 Bridge。
 */
internal fun shouldAlignGridOnSwitch(wantRows: Int, wantCols: Int, gridQuery: () -> Long): Boolean {
    val packed =
        try {
            gridQuery()
        } catch (exception: Exception) {
            // 查询失败与「查到 0」同解（都走无条件对齐），但必须留痕：
            // 静默吞掉会让「切会话后网格错位」无从追溯。
            LogUtil.error("Runtime", "grid query failed while switching session", exception)
            0L
        }
    return packed == 0L || (packed shr 32).toInt() != wantRows || packed.toInt() != wantCols
}

/**
 * 切会话的网格对齐执行：`[shouldAlignGridOnSwitch]` 为真才 `resize`。
 *
 * 独立成顶层函数是为了让判定逻辑可单测（`shouldAlignGridOnSwitch` 有同名单测）。
 */
internal fun alignGridOnSwitch(bridge: Bridge, rows: Int, cols: Int) {
    val wantRows = rows.coerceAtLeast(1)
    val wantCols = cols.coerceAtLeast(1)
    if (shouldAlignGridOnSwitch(wantRows, wantCols) { bridge.getGridRowsColsPacked() }) {
        bridge.resize(wantRows, wantCols)
    }
}
