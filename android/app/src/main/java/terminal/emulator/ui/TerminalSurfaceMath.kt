package terminal.emulator.ui

import terminal.emulator.SELECTION_BOUNDS_LENGTH
import terminal.emulator.bridge.Bridge
import terminal.emulator.settings.SettingsRepository
import kotlin.math.exp
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * 拖动 y 所在的边缘滚动区：上半格 → 上滚，下半格 → 下滚，中间 → 停止。
 * 与拖动处理器中的非对称边界一致（顶部 `<`、底部 `>=`），
 * 使退化 Surface（高度 < cellHeight）偏向顶部区。
 */
internal enum class EdgeScrollDirection {
    UP,
    DOWN,
    STOP,
}

internal fun edgeScrollDirection(pixelY: Float, surfaceHeightPx: Float, cellHeight: Float): EdgeScrollDirection = when {
    pixelY < cellHeight / 2 -> EdgeScrollDirection.UP
    pixelY >= surfaceHeightPx - cellHeight / 2 -> EdgeScrollDirection.DOWN
    else -> EdgeScrollDirection.STOP
}

/** 像素偏移 → 钳位后的网格单元格（0..maxCells-1；maxCells 为 0 时保持 0）。 */
internal fun pixelToCell(pixelX: Float, cellSize: Float, maxCells: Int): Int = (pixelX / cellSize).toInt().coerceIn(
    0,
    (maxCells - 1).coerceAtLeast(0),
)

/** 纯像素矩形（y 向下增大）：菜单锚定的几何输入，JVM 可单测（不碰 android.jar）。 */
internal data class PixelRect(val left: Int, val top: Int, val right: Int, val bottom: Int)

/**
 * 选区两端点按「先比行、行同再比列」归一化为 (起点, 终点)。
 *
 * 上游保留选择方向，`start`/`end` 可能反向。归一化只有这一处实现：菜单锚定与
 * 取文两处各写一份时曾分叉——取文那份判 `row < row || (row == row && col <= col)`，
 * 菜单那份少了行相等那一支，对向左下的选区会算出 leftCol > rightCol。今天菜单只取
 * `(left + right) / 2`，交换不可见，故未暴露；任何非对称用法（如按宽度裁剪）即继承该错。
 */
internal fun <T> normalizeSelectionAnchors(start: T, end: T, row: (T) -> Int, col: (T) -> Int): Pair<T, T> =
    if (row(start) < row(end) || (row(start) == row(end) && col(start) <= col(end))) {
        start to end
    } else {
        end to start
    }

/**
 * 菜单锚定纯函数（design 决策 3）：返回菜单左上角 (x, y)；无处可放返回 null（调用方隐藏菜单）。规则：
 * - 上方优先：菜单底缘高出选择顶缘一个手柄高（锚隙）；贴顶（越出视口上缘）则翻到选择底缘之下；
 * - 贴右钳制：水平居中于选择后夹进视口左右缘；
 * - 菜单必须整体落在视口内：任一落点越出视口即不可用（选区完全滚出视口时两处都越界）；
 * - 选择盖满视口 —— 上下两处落点都放不进选区外的剩余空间（等价于两处都会与选择相交）
 *   —— 返回 null：菜单任何时刻不遮挡选择。
 */
internal fun menuAnchor(
    selection: PixelRect,
    viewport: PixelRect,
    menuWidth: Int,
    menuHeight: Int,
    handleHeight: Int,
): Pair<Int, Int>? {
    if (viewport.right - viewport.left <= 0 || viewport.bottom - viewport.top <= 0) return null
    val width = menuWidth.coerceAtMost(viewport.right - viewport.left)
    val menuLeft = (
        (selection.left + selection.right) / 2 - width / 2
        ).coerceIn(viewport.left, (viewport.right - width).coerceAtLeast(viewport.left))

    // 两处落点共用同一条「整体在视口内」判据：只判单侧会让选区滚出视口时
    // 返回一个视口外的 y（PopupWindow 被添加到屏幕外，菜单不可见且不报错）。
    fun fits(top: Int) = top >= viewport.top && top + menuHeight <= viewport.bottom
    val aboveTop = selection.top - menuHeight - handleHeight
    if (fits(aboveTop)) {
        return menuLeft to aboveTop
    }
    val belowTop = selection.bottom + handleHeight
    if (fits(belowTop)) {
        return menuLeft to belowTop
    }
    return null
}

/**
 * 滚动到绝对网格行 [row] 时的目标偏移（单位：回滚行数）。
 *
 * 偏移把该行顶推到视口顶，故 `视口顶行 = scrollbackLength - 偏移 = row`；钳到
 * 0..scrollbackLength 使越界的行（低于回滚顶或高于末行）落到能显示的最远处而不
 * 产生负偏移。抽出为纯函数供 JVM 测试——`scrollToRow` 经它计算，测试也经它，
 * 测试里不再自带一份公式（自带时断言恒成立：把 [TerminalSurface.scrollToRow]
 * 整个删掉也判不了红）。
 */
internal fun scrollOffsetForRow(row: Int, scrollbackLength: Int): Int =
    (scrollbackLength - row).coerceIn(0, scrollbackLength)

/**
 * 搜索居中时命中行在视口中的目标行号。
 *
 * 可见带按**屏幕坐标**定义：输入法遮住底部一段像素、键栏占一段高度，内容又为跟随
 * 键盘整体上移一段位移。命中行的屏幕纵坐标 = 视口行 × cellHeight − 上移量；
 * 令其落在可见带正中即得本值。
 *
 * 只按「总行数 − 输入法行数」折算会漏掉上移量：输入法遮挡超过终端总高度的三分之一时，
 * 命中行会被推到**屏幕上缘之外**，用户仍看不到匹配文本。
 *
 * @param visibleContentPx 可见带高度（Surface 高度 − 键栏占高 − 输入法遮挡）。
 * @param shiftPx 内容跟随键盘整体上移的像素量。
 * @param cellHeightPx 单元高度（px）。必须是真实值：截断成整数会让折算出的视口行
 *   偏出数行（单元高 44.625 与 44 的差别在两千像素的折算量上是 6 行）。
 */
internal fun centerSearchViewportRow(visibleContentPx: Int, shiftPx: Int, cellHeightPx: Float): Int {
    if (cellHeightPx <= 0f) return 0
    val targetScreenYPx = visibleContentPx / 2f
    return ((targetScreenYPx + shiftPx) / cellHeightPx).roundToInt().coerceAtLeast(0)
}

/**
 * 把绝对回滚网格坐标 (row, col) 映射为视口像素坐标。
 * `viewportTopGrid` 是视口顶部显示的绝对网格行（scrollbackLength - scrollOffset）；
 * 对已是视口相对的行传 0。从内联的 `row - (scrollbackLength - scrollOffset)` 公式中抽出，
 * 使滚动、搜索跳转与字号变化共用一个可测试的换算。
 *
 * 返回未取整的像素；调用方自行取整并钳位到视图边界
 * （手柄锚定在行底部时传入 row + 1）。
 */
internal fun gridToScreen(
    row: Int,
    col: Int,
    viewportTopGrid: Int,
    cellWidth: Float,
    cellHeight: Float,
): Pair<Float, Float> = Pair(col * cellWidth, (row - viewportTopGrid) * cellHeight)

/**
 * 触点列 → 目标列：相对**抓柄时**的参考点做整格位移。
 *
 * 参考点由抓柄时握住的那一侧决定（首端柄 = 首格左缘，末端柄 = 末格右缘），
 * 整次拖动内保持不变。手柄越过另一侧时只换归属，参考点若跟着换，
 * 目标列会在翻转瞬间跳一格：边界反向移动、交叉被立刻撤销，
 * 被拖的柄也会与手指脱开一格（对标 termlib 固定 grabOffset 的做法）。
 */
internal fun dragTargetColumn(
    anchorCol: Int,
    anchorIsStart: Boolean,
    touchX: Float,
    cellWidth: Float,
    cols: Int,
): Int {
    val referenceLocalX = (if (anchorIsStart) anchorCol else anchorCol + 1) * cellWidth
    val deltaCols = ((touchX - referenceLocalX) / cellWidth).roundToInt()
    return (anchorCol + deltaCols).coerceIn(0, (cols - 1).coerceAtLeast(0))
}

/**
 * 触点行 → 目标行（视口行）。
 *
 * 手柄悬于格底，锚点是格底缘，故触点所在行 = 锚点行 + 手指的整行位移。
 * [anchorViewportRow] 是锚点行的视口行号；锚点行与触点 Y 同处视口坐标系，
 * 两者相减即为手指位移，绝对网格行由调用方加上视口顶行。
 */
internal fun dragTargetRow(anchorViewportRow: Int, touchY: Float, cellHeight: Float, rows: Int): Int {
    val referenceLocalY = (anchorViewportRow + 1) * cellHeight
    val deltaRows = ((touchY - referenceLocalY) / cellHeight).roundToInt()
    return (anchorViewportRow + deltaRows).coerceIn(0, (rows - 1).coerceAtLeast(0))
}

/** 组字光标的插入标记矩形（视图局部坐标，平台随后按视图矩阵变换到屏幕）。 */
internal data class InsertionMarker(val horizontal: Float, val top: Float, val baseline: Float, val bottom: Float)

/**
 * 组字光标锚点的标记矩形；光标不在视口内时返回 null（调用方据此不回报）。
 *
 * 另两条准入（输入法正在请求光标监视、当前有组字）在调用前判定：它们只依赖
 * 输入法与组字基线，不需要本次的网格坐标，先判掉就省下那次同步 VT 查询。
 *
 * 取**终端光标格**而非组字文本自身位置，因为组字字节正写进那一格。
 */
internal fun composingCursorMarker(
    cursorRow: Int,
    cursorCol: Int,
    rows: Int,
    cellWidth: Float,
    cellHeight: Float,
): InsertionMarker? {
    if (cursorRow < 0 || cursorRow >= rows || cursorCol < 0) return null
    val (markerX, markerY) = gridToScreen(cursorRow, cursorCol, 0, cellWidth, cellHeight)
    return InsertionMarker(
        horizontal = markerX,
        top = markerY,
        baseline = markerY + cellHeight,
        bottom = markerY + cellHeight,
    )
}

/**
 * 规范化的选区边界：start ≤ end，两个锚点均钳位到网格内。
 * 由 [clampSelection] 产出；供拖动手柄更新路径消费，
 * 使原生 setSelection 绝不看到倒置或越界的单元格。
 */
internal data class SelectionBounds(val startRow: Int, val startCol: Int, val endRow: Int, val endCol: Int)

/**
 * 词选边界解析：取上游 `select_word` 派生的有序界限，失败时退化为落点单格。
 *
 * 两条词选入口（长按、侧键点击）共用本函数，使回退形态不可能分叉：回退分支若只在
 * 其中一条路径上遗漏 `endSelection`，选区会永久停在拖动态，此后输入不再自动贴底。
 */
internal fun resolveWordSelectionBounds(wordBounds: IntArray?, gridRow: Int, col: Int): SelectionBounds =
    if (wordBounds != null && wordBounds.size == SELECTION_BOUNDS_LENGTH) {
        SelectionBounds(wordBounds[0], wordBounds[1], wordBounds[2], wordBounds[3])
    } else {
        SelectionBounds(startRow = gridRow, startCol = col, endRow = gridRow, endCol = col)
    }

/**
 * 保序的范围钳位（termux TextSelectionCursorController.updatePosition 语义）：
 *
 * 1. 把每个锚点钳入 `[0,maxRow] × [0,maxCol]`（max 为负 → 空网格 → 全零）；
 * 2. 输入倒置时交换锚点，使返回的边界始终满足 start ≤ end。
 *
 * 纯函数；支撑 SelectionManager.dragSelection 中拖动路径的纵深防御。
 */
internal fun clampSelection(
    startRow: Int,
    startCol: Int,
    endRow: Int,
    endCol: Int,
    maxRow: Int,
    maxCol: Int,
): SelectionBounds {
    val maxRowLimit = maxRow.coerceAtLeast(0)
    val maxColumnLimit = maxCol.coerceAtLeast(0)
    val clampedStartRow = startRow.coerceIn(0, maxRowLimit)
    val clampedStartColumn = startCol.coerceIn(0, maxColumnLimit)
    val clampedEndRow = endRow.coerceIn(0, maxRowLimit)
    val clampedEndColumn = endCol.coerceIn(0, maxColumnLimit)
    return if (clampedEndRow < clampedStartRow ||
        (clampedEndRow == clampedStartRow && clampedEndColumn < clampedStartColumn)
    ) {
        // 输入倒置：交换以使钳位后 start ≤ end。
        SelectionBounds(
            startRow = clampedEndRow,
            startCol = clampedEndColumn,
            endRow = clampedStartRow,
            endCol = clampedStartColumn,
        )
    } else {
        SelectionBounds(
            startRow = clampedStartRow,
            startCol = clampedStartColumn,
            endRow = clampedEndRow,
            endCol = clampedEndColumn,
        )
    }
}

/**
 * 单元格列是否为空白（长按仅弹出粘贴菜单；支撑 handleLongPress）。
 *
 * [line] 是 [Bridge.scrollbackLine] 的原样结果：**每列恰好一个字符**，宽字符尾格是
 * 空格占位，故字符下标即列号，直接取 `line[col]` 即可——按宽度表反推列↔字符映射
 * 会在每个宽字符之后整体错位一格（该模型已删除，见归档变更 design 第 1 节）。
 * 宽字符尾格的落点须先经 `snapToWideCharBoundary` 吸附到字符起始列再传入。
 *
 * 空行（null）、空白单元、或行尾之后的任何列都算空白：termux 的
 * `getSelectedText(x,y,x,y)` 对三者都返回 ""，故它们都必须归类为仅粘贴——
 * 原先的 `col < line.length` 合取条件把行尾各列归类为文本，并在那里弹出带
 * PASTE 的完整菜单（根因 A1）。
 */
internal fun isWhitespaceCell(line: String?, col: Int): Boolean = line?.getOrNull(col)?.isWhitespace() != false

/** 手柄拖动结束后的保护窗：期间在松手位置的轻点被吞掉，而不是关闭刚重新显示的菜单（termux 隐藏保护）。 */
internal const val SELECTION_MENU_RESHOW_GUARD_MS = 300L

/**
 * [nowMs] 是否落在上次拖动结束（[lastDragEndMs]，uptimeMillis）后的
 * [SELECTION_MENU_RESHOW_GUARD_MS] 之内。拖动后的首次轻点被视为手势收尾的一部分，
 * 而非「点击选区外 → 关闭」的命令。纯函数；支撑 onSingleTapUp。
 */
internal fun shouldSuppressTapAfterDragEnd(
    nowMs: Long,
    lastDragEndMs: Long,
    windowMs: Long = SELECTION_MENU_RESHOW_GUARD_MS,
): Boolean = lastDragEndMs > 0L && nowMs >= lastDragEndMs && nowMs - lastDragEndMs < windowMs

/** 菜单“打开链接”目标：仅 OSC 8 超链接 URI。纯文本裸 URL 不做识别
 * （libghostty-vt 只提供 OSC 8，不含纯文本 URL 扫描）。显示侧以本函数的
 * 非空性判定，动作与显示绝不分叉。
 */
internal fun resolveOpenLinkUri(hyperlinkUri: String?): String? = hyperlinkUri?.trim()?.takeIf { it.isNotEmpty() }

/** 滚动行高下限：避免除零，保持手势可用。 */
internal const val MIN_CELL_HEIGHT_PX = 1f

/** 滚动一步结果：钳制后的偏移、剩余亚行余量、更新后的位移显出量。 */
internal data class ScrollStep(
    val newOffset: Int,
    val newAccumulatorPx: Float,
    /** 更新后的位移显出量（见 `TerminalSurface.imeShiftRevealPx`）。 */
    val revealPx: Float = 0f,
)

/**
 * 滚动偏移重基（纯函数，见 [TerminalSurface.rebaseScrollOffset]）。
 *
 * 增长量取回滚长度的增量：内容在底边增长了多少行，同一处内容就离底边远了多少行，
 * 偏移必须同步增长才仍指向同一处内容。偏移为 0（视口贴底）时不动——贴底时视口随
 * 新输出下移，偏移恒为 0。
 */
internal fun rebasedScrollOffset(offset: Int, scrollbackLength: Int, previousScrollbackLength: Int): Int {
    val growth = scrollbackLength - previousScrollbackLength
    if (growth == 0 || offset == 0) return offset
    return (offset + growth).coerceIn(0, scrollbackLength)
}

/**
 * 手指滚动增量换算（onScroll 可测核心，同向逻辑）。
 * distanceY 为手势约定（previousY - currentY）：下移为负，进入更早历史（偏移增加）；
 * 上移为正，回到更新内容（偏移减少）。亚行余量累积，整行才移动；边缘钳制并清余量。
 *
 * 边界必须先判，且排在亚行早退之前：已在边界（偏移 0 或回滚长度）继续向边界外拖时，
 * 余量若照常下发，渲染器会把整幅网格按投影亚行平移，边缘露出一条背景空带；等余量
 * 攒够一整行才被清零，内容又整体弹回一行——即「滑到边界跳动闪烁」。故越界位移一律
 * 就地丢弃，不经过渲染器。
 *
 * 回滚区顶端（[scrollbackLength]）还有一层越界：位移把网格顶部裁在屏幕上边界之外，
 * 继续上滑时越界位移喂给 [revealPx]，按像素把那段顶部放回屏幕；反向（回到边界内）
 * 先按同一比例回收显出量再下滚，两个方向都是 1:1。
 *
 * 取整使用截断趋向零（toInt），正/负余量的亚行阈值对称：
 * floor 在负方向过激（-0.9px → -1 行而 +0.9px → 0 行），
 * 导致连续拖动累加 1 行级漂移；toInt 两端均需超过 1 行才触发行变。
 */
internal fun applyScrollDistance(
    accumulatorPx: Float,
    distanceY: Float,
    cellHeightPx: Float,
    scrollOffset: Int,
    scrollbackLength: Int,
    revealPx: Float = 0f,
    revealCapacityPx: Float = 0f,
): ScrollStep {
    var accumulator = accumulatorPx - distanceY
    val cellHeight = cellHeightPx.coerceAtLeast(MIN_CELL_HEIGHT_PX)
    var reveal = revealPx
    if (scrollOffset >= scrollbackLength && accumulator < 0f) {
        // 回到边界内：先 1:1 回收显出量，回收完（余量为 0）才真正下滚。
        val consumed = minOf(-accumulator, reveal)
        reveal -= consumed
        accumulator += consumed
    }
    if (accumulator > 0f && scrollOffset >= scrollbackLength) {
        // 越过回滚区顶端：先按容量显出被位移裁掉的网格顶部，显不出的部分直接丢弃。
        val room = (revealCapacityPx - reveal).coerceAtLeast(0f)
        val revealed = minOf(accumulator, room)
        // 显不出的部分（accumulator - revealed）就地丢弃，不跨行回弹。
        return ScrollStep(scrollOffset, 0f, reveal + revealed)
    }
    if (accumulator < 0f && scrollOffset <= 0) {
        return ScrollStep(scrollOffset, 0f, revealPx = reveal)
    }
    // 截断趋向零：正/负亚行余量对称，消除 floor 非对称导致的 1 行级漂移
    val rawAmount = (accumulator / cellHeight).toInt()
    if (rawAmount == 0) {
        return ScrollStep(scrollOffset, accumulator, revealPx = reveal)
    }
    val newOffset = (scrollOffset + rawAmount).coerceIn(0, scrollbackLength)
    accumulator -= (newOffset - scrollOffset) * cellHeight
    if ((newOffset == 0 && accumulator < 0f) || (newOffset == scrollbackLength && accumulator > 0f)) {
        accumulator = 0f
    }
    return ScrollStep(newOffset, accumulator, revealPx = reveal)
}

/**
 * 惯性模型的时间常数（毫秒）：速度按 `v(t) = v₀·e^(-t/τ)` 衰减，
 * 总行程为 `v₀·τ`（像素）。350ms 的观感与平台样条接近（5000px/s ≈ 1750px 行程，
 * 约 1.5s 内走完 99%），但完全由本仓决定，不受厂商改写影响。
 */
internal const val FLING_TIME_CONSTANT_MS = 350f

/** 惯性动画的时长上限（毫秒）：此后位移增量已不足半像素，继续发 vsync 回调没有意义。 */
internal const val FLING_MAX_DURATION_MS = 2450L

/** 剩余行程小于该值即视为衰减完毕（像素）。 */
internal const val FLING_SETTLE_PX = 0.5f

/**
 * 惯性位移（onFling 可测核心）：给定抬起速度和已过时间，返回**相对起点的行程**（像素）。
 *
 * 纯函数是刻意选的：平台 [android.widget.OverScroller] 会被厂商 ROM 改写，
 * 真机日志里整段样条被"优化"成一帧落点（`force Finished`），一次猛甩直接跳十几行。
 * 指数衰减跨设备一致、单调、有界（上界 `v₀·τ`），且能逐帧落到亚像素，
 * 配合 [applyScrollDistance] 的余量实现逐像素跟随。
 *
 * 单位约定与手势一致：手指上移速度为负，行程也为负（偏移变小、回到更新内容）。
 */
internal fun flingOffsetPx(velocityPxPerSecond: Float, elapsedMs: Long): Float {
    if (elapsedMs <= 0L) return 0f
    val decayed = 1f - exp(-elapsedMs / FLING_TIME_CONSTANT_MS)
    return velocityPxPerSecond * (FLING_TIME_CONSTANT_MS / 1000f) * decayed
}

/**
 * 缩放手势字号换算（onScale 可测核心）。
 * 基准字号乘以累计缩放因子后收敛到**可设档位**（区间 + 步长网格），与调节条同一入口。
 *
 * 收敛到档位而非只钳上下界：档位网格是调节条能给出的全部取值，手势若给出非档位值
 * （如 17.6sp），落盘后滑块读数按整数打印（显示「18 SP」）而实际渲染的是 17.6sp，
 * 展示与生效分家；拇指停在非档位位置，用户触碰一次滑块还会被改写成档位值。
 *
 * @param spToPxScale sp→像素系数（见 `TerminalRuntime.spToPxScale`），非仅显示密度：
 *   调节条上限与之同源，两者用不同系数会让手势越出调节条允许的区间。
 * @param screenWidthDp 屏幕宽（dp）：上界须与调节条按同一个屏幕推导，否则捏合能捏到
 *   调节条给不出的字号——用户反馈的「设置条与实际可设范围不一致」正是这个分叉。
 */
internal fun zoomFontSize(baseFontSizeSp: Float, scaleFactor: Float, spToPxScale: Float, screenWidthDp: Float): Float =
    SettingsRepository.clampFontSize(
        baseFontSizeSp * scaleFactor,
        spToPxScale,
        screenWidthDp,
    )

/**
 * 缩放手势是否收敛到新字号（onScaleEnd 可测核心）。
 * 基准与终值同为档位值（[SettingsRepository.FONT_SIZE_MIN_SP] 起、步长
 * [SettingsRepository.FONT_SIZE_STEP_SP]），真实变化至少相差一整档，故相等即「未变化」，
 * 不需要容差阈值。
 */
internal fun zoomSettledOnNewSize(baseFontSizeSp: Float, finalSizeSp: Float): Boolean = finalSizeSp != baseFontSizeSp

/**
 * 遮罩帧在开孔里的等比贴合参数（开孔遮罩的取放规则）。
 *
 * 遮罩帧是**离开画面时**回读的旧尺寸，而开孔尺寸会随后变化：输入法弹出/收起
 * 都会改 surface 高度。若要求尺寸完全一致才画，输入法收起那一刻遮罩整块作废，
 * 开孔退回平底色——用户看到的正是「输入法弹出时切后台返回终端闪烁」。
 *
 * 故按宽度等比缩放、顶边对齐：顶部是同一段终端内容（等比缩放不拉伸字形），
 * 高出开孔的部分裁掉，差下的部分由终端底色补。
 *
 * @param coverWidth 遮罩帧宽（位图像素）
 * @param coverHeight 遮罩帧高（位图像素）
 * @param viewWidth 当前开孔宽（视图像素）
 * @param viewHeight 当前开孔高（视图像素）
 * @return 贴合参数；尺寸非法（任一为 0 或负）时返回 null（调用方不画遮罩）
 */
internal fun coverFit(coverWidth: Int, coverHeight: Int, viewWidth: Int, viewHeight: Int): CoverFit? {
    if (coverWidth <= 0 || coverHeight <= 0 || viewWidth <= 0 || viewHeight <= 0) return null
    val scale = viewWidth.toFloat() / coverWidth.toFloat()
    val maxSourceHeight = viewHeight / scale
    val sourceHeight = min(coverHeight.toFloat(), maxSourceHeight)
    return CoverFit(sourceHeight = sourceHeight, destHeight = sourceHeight * scale)
}

/**
 * 遮罩帧的贴合参数：源矩形高度与目标矩形高度（均为像素）。
 *
 * 源矩形从位图左上角起、高 [sourceHeight]；目标矩形从视图左上角起、高 [destHeight]。
 */
internal data class CoverFit(
    /** 源矩形高度（位图像素）：等比缩放后仍超出开孔的部分被裁掉。 */
    val sourceHeight: Float,
    /** 目标矩形高度（视图像素）。 */
    val destHeight: Float,
)
