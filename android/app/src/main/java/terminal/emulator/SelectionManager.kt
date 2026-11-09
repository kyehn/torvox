package terminal.emulator

import android.content.Context
import android.content.Intent
import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.runtime.pasteByteChunks
import terminal.emulator.runtime.truncatePasteText
import terminal.emulator.util.TerminalDispatchers

/** 剪贴板文本上限：超出部分截断，避免一次性搬运超大文本。 */
private const val CLIPBOARD_TEXT_MAX_LENGTH = 100_000

/**
 * 选区状态机：开始/更新/结束、拖动手柄、剪贴板复制/分享/粘贴、全选。
 *
 * 从 [TerminalViewModel] 拆出：选区只依赖终端状态、运行期与剪贴板，
 * 不掺入会话、主题、引导等职责。状态经构造传入的 [MutableStateFlow] 读写，
 * 粘贴字节出口 [pasteSink] 由 Surface 侧注入批缓冲后替换。
 */
internal class SelectionManager(
    private val terminalState: MutableStateFlow<TerminalState>,
    private val runtime: TerminalRuntime,
    private val context: Context,
    private val scope: CoroutineScope,
) {
    private val clipboardAccess = ClipboardAccess(context, tag = "SelectionManager")

    /** 粘贴字节出口：默认同步写，Surface 侧注入批缓冲后走异步合并写。会话 id 随块传入，不在刷写时解析。 */
    var pasteSink: (Long, ByteArray) -> Unit = { sessionId, data -> runtime.writeToPty(sessionId, data) }

    fun startSelection(row: Int, col: Int, pasteOnly: Boolean = false) {
        val anchor = SelectionAnchor(row, col)
        // 新一次长按必须丢弃上一轮快速拖动留下的实时边界，否则首个 MOVE
        // 会以旧选区为基准算出错误端点。
        lastDragBounds = null
        // CAS：选区在主线程上被触碰，但 _state 也被 IO 协程写入；
        // 普通读-改-写可能丢失其中之一的更新。mode 在 lambda 内读取。
        terminalState.update { state ->
            state.copy(
                selection =
                SelectionState(
                    active = true,
                    dragging = true,
                    start = anchor,
                    end = anchor,
                    pasteOnly = pasteOnly,
                    menuDismissed = false,
                ),
            )
        }
    }

    fun updateSelection(row: Int, col: Int) = dragSelection(row = row, col = col)

    /**
     * 快速拖动路径：不触碰 Compose 状态地计算选区边界。
     * 返回归一化后的拖动结果（含翻转后的归属柄）供直接重定位手柄；
     * 拖动未激活或仅粘贴时返回 null。
     *
     * 调用方必须依返回的 `draggingStart` 更新被拖柄归属，否则越过锚点后的
     * 连续回拖会按旧柄解释（对标 termlib applyHandleDrag 五元组语义）。
     * 调用方直接依返回的边界重定位手柄，避开 Compose _state
     * → 重组 → 回读的往返，而这正是每次 MOVE 的主要帧瓶颈。
     */
    fun dragMove(draggingStart: Boolean, row: Int, col: Int, cachedMaxRow: Int, cachedMaxCol: Int): HandleDragResult? {
        val current = terminalState.value.selection
        if (!current.active || current.pasteOnly) return null
        val flipped =
            resolveDragBounds(
                current = current,
                pending = lastDragBounds,
                draggingStart = draggingStart,
                row = row,
                col = col,
                maxRow = cachedMaxRow,
                maxCol = cachedMaxCol,
            )
        lastDragBounds = flipped
        syncDragBoundsToNativeThrottled(flipped)
        return flipped
    }

    /**
     * 以节流频率把进行中的拖动边界推送到原生渲染器，使单元格反色高亮实时跟随手柄
     * （termux updatePosition 对等），而无需在每个 MOVE 帧都付出一次 JNI 穿越与重渲染。
     * 最终的精确边界由松手时 endSelection 经 runtime.setSelection 提交。
     *
     * 下发的是选区自身的两个端点（行主序有序，见 [resolveDragBounds] 的钳位），
     * 不是行列各自取极值的包围盒角点：原生按线性语义解释端点，角点会让高亮区域
     * 在首行左侧与末行右侧同时外扩，而手柄仍停在端点上。
     */
    private fun syncDragBoundsToNativeThrottled(bounds: HandleDragResult) {
        val nowMs = SystemClock.uptimeMillis()
        if (nowMs - lastDragNativeSyncUptimeMs < DRAG_NATIVE_SYNC_INTERVAL_MS) return
        lastDragNativeSyncUptimeMs = nowMs
        // 走拖动途中的专用入口：普通提交会把 dragging 清成 false，
        // 那正是「保护只存活一个推送间隔」的由来（见 TerminalRuntime 的同名方法）。
        runtime.pushSelectionBoundsDuringDrag(
            bounds.startRow,
            bounds.startCol,
            bounds.endRow,
            bounds.endCol,
        )
    }

    @Volatile private var lastDragNativeSyncUptimeMs = 0L

    /**
     * 在 endSelection 之前把最后一次 dragMove 的边界提交到 Compose 状态。
     * 必须由 finishHandleDrag 调用，使 endSelection 读到正确的边界。
     */
    fun commitDragBounds() {
        val bounds = lastDragBounds ?: return
        terminalState.update { state ->
            val currentSelection = state.selection
            if (!currentSelection.active) return@update state
            state.copy(
                selection =
                currentSelection.copy(
                    start = SelectionAnchor(bounds.startRow, bounds.startCol),
                    end = SelectionAnchor(bounds.endRow, bounds.endCol),
                ),
            )
        }
        lastDragBounds = null
    }

    private var lastDragBounds: HandleDragResult? = null

    /** 慢速路径：直接写 Compose 状态地拖动末端边界（遗留触摸路径与仪器化用例使用）。 */
    private fun dragSelection(row: Int, col: Int) {
        // 拖动路径钳位所用的绝对网格边界（见下）；只读一次——
        // 并发的 resize 最多使该边界陈旧一帧。
        val runtimeState = runtime.state.value
        val maxRow = absoluteMaxRow(runtimeState.rows)
        val maxCol = (runtimeState.cols - 1).coerceAtLeast(0)
        // CAS，活跃性检查置于 lambda 内部：选区读取与写入相对并发的
        // _state 更新是原子的。仅粘贴的选区（空单元格/空白处长按）不可变
        // ——长按期间手指的微小移动绝不能使该单格漂移。
        terminalState.update { state ->
            val current = state.selection
            if (!current.active || current.pasteOnly) {
                state
            } else {
                // 与快速路径共用同一份「当前边界」：拖动途中的边界可能尚未提交到
                // _state（快速路径刻意不写），直接拿已提交的旧端点当基准会把
                // **没被拖动**的一侧算到错误位置——快速路径与边缘滚动交替时，
                // 现象就是「另一边界异常增加」。
                val flipped =
                    resolveLongPressDragBounds(
                        current = current,
                        pending = lastDragBounds,
                        row = row,
                        col = col,
                        maxRow = maxRow,
                        maxCol = maxCol,
                    )
                // 两条路径都写同一份实时边界，下一帧无论走哪条都以它为基准。
                lastDragBounds = flipped
                state.copy(
                    selection =
                    current.copy(
                        dragging = true,
                        start = SelectionAnchor(flipped.startRow, flipped.startCol),
                        end = SelectionAnchor(flipped.endRow, flipped.endCol),
                        // 拖动期间隐藏浮动菜单；它在 ACTION_UP 时
                        // 于新位置重新出现（endSelection 恢复 menuDismissed=false）。
                        menuDismissed = true,
                    ),
                )
            }
        }
        // 在运行期标记拖动进行中，使渲染线程在手柄移动期间
        // 抑制新输出引起的滚动复位（termux skipScrolling 对等）。
        // 由 endSelection/clearSelection 经 runtime.setSelection 清除。
        if (terminalState.value.selection.dragging) {
            runtime.setSelectionDragging(true)
        }
    }

    fun endSelection() {
        val current = terminalState.value.selection
        if (!current.active || current.start == null || current.end == null) return
        val text = extractSelectedText(current)
        // 仅当自读取以来选区未变才写入：并发的选区更新绝不能被陈旧文本覆盖。
        // CAS 循环：compareAndSet 在状态仍属于我们时重试，
        // 一旦有并发写入落地即中止——下方的原生边界同步只对真正提交的快照运行
        // （没有可能跨重试泄漏的副作用标志）。
        // 注意：extractSelectedText 自身也是跨快照读取 bridge 与 cols
        // ——若 IO 协程在提取中途切换会话，这些同样可能陈旧；
        // 结果受子串守卫限制，且除非本次 CAS 提交否则绝不写入。
        val updated = current.copy(dragging = false, selectedText = text, menuDismissed = false)
        while (true) {
            val state = terminalState.value
            if (state.selection != current) return
            if (terminalState.compareAndSet(state, state.copy(selection = updated))) break
        }
        publishNormalizedSelectionBounds(current.start, current.end)
    }

    /**
     * 选区文本的**唯一**解析入口：菜单可用性判定与菜单动作共用它。
     *
     * 缓存为空时按需重取——原生 `selection_text` 在部分回滚/重排状态下返回空，
     * 此时动作仍必须拿到真实文本。共用同一解析是「复制按钮毫无反应」的根治点：
     * 只要某一侧独立判定，就会出现「项显示、动作却取到空串 → 静默返回」的分叉
     * （实测：长按落在有内容的格子、抽取范围却整段落在行尾空白列上，
     * 修剪后为空串，点复制后剪贴板不变、菜单照样关闭）。
     */
    fun resolvedSelectionText(): String {
        val selection = terminalState.value.selection
        if (!selection.active) return ""
        // 重新提取而不信任缓存的 selectedText：若缓存文本为空
        // （某些回滚状态下原生 selection_text 为 null），
        // 按需提取保证菜单动作总是复制当前选区。
        return selection.selectedText.ifEmpty { extractSelectedText(selection) }
    }

    /** 选区边界的日志形式：空解析告警必须自带定位信息，否则等于没记。 */
    private fun selectionBoundsForLog(): String {
        val selection = terminalState.value.selection
        val start = selection.start
        val end = selection.end
        if (start == null || end == null) return "(无界)"
        return "(start=${start.row},${start.col} end=${end.row},${end.col} pasteOnly=${selection.pasteOnly})"
    }

    fun copySelectionToClipboard() {
        val rawText = resolvedSelectionText()
        if (rawText.isEmpty()) {
            // 不再静默：空解析是用户可见的「点了没反应」，必须留痕可查。
            LogUtil.warn("SelectionManager", "复制跳过: 选区文本为空 ${selectionBoundsForLog()}")
            return
        }
        val clipped = rawText.take(CLIPBOARD_TEXT_MAX_LENGTH)
        clipboardAccess.setClipboardText(clipped, label = "terminal selection")
        // 动作完成后关闭浮动菜单；保留高亮。
        terminalState.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
    }

    fun clearSelection() {
        // 同批作废未提交的拖动边界：输入法显隐、surface 销毁等路径会在拖动途中
        // 清选区，`finishHandleDrag` 不再到达。此后新建的选区若再提交一次拖动，
        // `commitDragBounds` 会把**上一段**的旧边界写进来，抽取与复制随即取到
        // 不相干的内容。
        lastDragBounds = null
        terminalState.update { it.copy(selection = SelectionState()) }
        syncSelectionToNative()
        // 强制立即重绘：否则原生侧的脏标志要等下一个 vsync 节拍，
        // 使陈旧的选区高亮在屏幕上残留长达一帧周期。
        runtime.forceRender()
    }

    private fun syncSelectionToNative() {
        val selection = terminalState.value.selection
        if (selection.active && selection.start != null && selection.end != null) {
            publishNormalizedSelectionBounds(selection.start, selection.end)
        } else {
            runtime.setSelection(0, 0, 0, 0, false)
        }
    }

    /**
     * 选区边界同步到原生：下发行主序有序的端点对（见 [selectionEndpoints]），
     * 调用方保证非空。
     */
    private fun publishNormalizedSelectionBounds(start: SelectionAnchor, end: SelectionAnchor) {
        val bounds = selectionEndpoints(start, end)
        runtime.setSelection(
            bounds.startRow,
            bounds.startCol,
            bounds.endRow,
            bounds.endCol,
            true,
        )
    }

    fun shareSelection() {
        val rawText = resolvedSelectionText()
        if (rawText.isEmpty()) {
            LogUtil.warn("SelectionManager", "分享跳过: 选区文本为空 ${selectionBoundsForLog()}")
            return
        }
        val shareIntent =
            Intent.createChooser(
                Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_TEXT, rawText)
                },
                null,
            )
        shareIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(shareIntent)
        // 动作完成后关闭浮动菜单。
        terminalState.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
    }

    fun selectAll() {
        // 上游 select_all：整个内容（回滚 + 视口）一次派生，native 侧已
        // 安装为终端选区，Kotlin 只消费回传的有序界限（不含尾部空行）。
        val bounds = runtime.bridge()?.selectAll()
        if (bounds == null || bounds.size != SELECTION_BOUNDS_LENGTH) {
            // 无内容/查询失败：按“无数据”处理，不伪造选择。显式记录：
            // 静默返回让调用方无法区分空缓冲与查询超时。
            LogUtil.warn("SelectionManager", "selectAll: no bounds (bridge/request failed), skipping")
            return
        }
        val selectionState =
            SelectionState(
                active = true,
                dragging = false,
                start = SelectionAnchor(row = bounds[0], col = bounds[1]),
                end = SelectionAnchor(row = bounds[2], col = bounds[3]),
            )
        val text = extractSelectedText(selectionState)
        terminalState.update { it.copy(selection = selectionState.copy(selectedText = text)) }
        syncSelectionToNative()
    }

    private fun extractSelectedText(selection: SelectionState): String {
        val start = selection.start ?: return ""
        val end = selection.end ?: return ""
        val bridge = runtime.bridge()
        if (bridge == null) {
            LogUtil.error("SelectionManager", "extractSelectedText: 无活动会话，选区文本未提取")
            return ""
        }
        // 选区行以绝对网格坐标存储（0 = 回滚顶部），与 Ghostty 格式化器的网格行一致。
        //
        // 换行感知的提取（termux TerminalBuffer.getSelectedText 语义）：
        // 软换行行拼接时不插入
        // 软换行处不插入 '\n'（解包）并去除尾部空白，
        // 且格式化器在内部把网格列映射到字符下标，使 CJK 宽字符绝不被切开
        // （等效于 TerminalRow.findStartOfColumn）。
        // 旧的逐行 scrollbackLine + '\n' 拼接无法侦测换行，且可能切开代理对。
        val bounds = selectionEndpoints(start, end)
        // 空选区返回 ""，查询失败返回 null——后者必须出声：合并成 "" 会让
        // 复制按钮无声失败，用户只看到「选中了却看不出选中」（无高亮变化）。
        val text =
            bridge.selectionText(
                bounds.startRow,
                bounds.startCol,
                bounds.endRow,
                bounds.endCol,
            )
        if (text == null) {
            LogUtil.error("SelectionManager", "extractSelectedText: 原生选区查询失败，选区文本未提取")
            return ""
        }
        return text
    }

    /**
     * 把剪贴板内容直接粘贴到 PTY（无确认对话框）。
     *
     * **必须**离开主线程：`encodePaste` 是同步 JNI，100 万字符剪贴板要整块转
     * UTF-8 并经上游 `paste::encode` 重新编码，仓库自述上界 500ms
     * （`QUERY_TIMEOUT_MS`），而它是菜单与侧键按钮的点击回调。
     *
     * 粘贴走 `pasteSink`（Surface 侧注入 `InputBatchBuffer.write`）：100 万字符
     * 剪贴板产生 250 块，主线程逐块同步写 PTY 会阻塞在会话锁 + PTY 写入上（N1-26）。
     * ViewModel 自身无批缓冲，sink 由调用方（Surface 侧）注入；它内部加锁，
     * 故从 IO 线程调用无碍。
     *
     * 返回值曾是「粘贴字符数」，三个调用点一个都没用，故去掉。
     */
    fun pasteFromClipboard() {
        scope.launch(TerminalDispatchers.inputOutput) {
            // 空剪贴板与读取失败都不许无声：失败已由 ClipboardAccess 记日志，
            // 这里补的是「点粘贴却什么都不发生」的空剪贴板。
            val text =
                clipboardAccess.clipboardText().getOrNull()
                    ?: run {
                        LogUtil.warn("SelectionManager", "粘贴跳过: 剪贴板无文本")
                        return@launch
                    }
            executePaste(text)
        }
    }

    /** 把 [text] 编码后真正写入 PTY。
     *
     * 主线程禁直接写：100 万字符剪贴板产生 250 块 JNI 同步写，
     * 阻塞在会话锁 + PTY 写入上。调用方须经 `InputBatchBuffer` 入队。
     *
     * 编码经上游 `paste::encode`（原生 `encodePaste`）：bracketed 包裹与否
     * 取自终端模式 2004 的活体状态，控制字节由上游中化，本处不解释内容。
     * 编码失败（会话已消失等）记警告后跳过：后续写入同样落空。
     */
    private fun executePaste(text: String) {
        // 目标会话在粘贴开始时定一次：分块入队是异步刷写，期间切会话
        // 会让粘贴尾部写进新会话。
        val sessionId = runtime.inputTargetSessionId
        val truncated = truncatePasteText(text)
        if (truncated.isEmpty()) {
            LogUtil.debug("SelectionManager", "粘贴跳过：空白或截断后无内容")
            terminalState.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
            return
        }
        val encoded =
            try {
                NativeBridge.encodePaste(sessionId, truncated.toByteArray(Charsets.UTF_8))
            } catch (exception: Exception) {
                LogUtil.warn("SelectionManager", "粘贴编码失败", exception)
                null
            }
        if (encoded == null || encoded.isEmpty()) {
            LogUtil.warn("SelectionManager", "粘贴跳过：编码无输出")
            terminalState.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
            return
        }
        // `pasteSink` 由 Surface 注入批缓冲（内部 `synchronized`），任意线程可调。
        for (chunk in pasteByteChunks(encoded)) {
            pasteSink(sessionId, chunk)
        }
        terminalState.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
    }

    // 视图侧回写的回滚长度缓存：选区行是绝对行（0 = 回滚顶部），
    // 钳位上界必须是绝对空间最后一行（回滚长度 + 视口行数 - 1），不得用视口行数。
    @Volatile private var cachedScrollbackLength = 0

    /** 绝对空间的最后一行：两处选区钳位的唯一来源，避免各自计算再次漂移。 */
    private fun absoluteMaxRow(rows: Int): Int = (cachedScrollbackLength + rows - 1).coerceAtLeast(0)

    /**
     * 视图侧每次刷新回滚长度后回写，使选区钳位与视图用同一口径。
     *
     * 刻意不另起一个 `StateFlow` 镜像：组合侧没有订阅者（重组期间自己发起同步 JNI
     * 查询是最长可阻塞两次查询超时（约 1s）的调用），镜像只会在每次手势查询时多发一次通知。
     */
    fun updateScrollbackLength(length: Int) {
        cachedScrollbackLength = length
    }

    /** 网格 resize 后把选区锚点钳位到绝对网格，使原生 setSelection 绝不收到越界单元格。 */
    fun clampSelectionToGrid(selection: SelectionState, rows: Int, cols: Int): SelectionState {
        val start = selection.start ?: return selection
        val end = selection.end ?: return selection
        val maxRow = (cachedScrollbackLength + rows - 1).coerceAtLeast(0)
        val maxCol = (cols - 1).coerceAtLeast(0)
        return selection.copy(
            start =
            start.copy(
                row = start.row.coerceIn(0, maxRow),
                col = start.col.coerceIn(0, maxCol),
            ),
            end =
            end.copy(
                row = end.row.coerceIn(0, maxRow),
                col = end.col.coerceIn(0, maxCol),
            ),
        )
    }

    /**
     * termux setInitialTextSelectionPosition 流程：在空白单元格/仅粘贴的选区上抓住手柄时，
     * 将其升级为普通文本选区，使随后的拖动能*扩展*范围，而不被「仅粘贴不可变」守卫吞掉。
     */
    fun beginHandleDragOnPasteOnly() {
        terminalState.update { state ->
            val current = state.selection
            if (!current.active || !current.pasteOnly) {
                state
            } else {
                state.copy(selection = current.copy(pasteOnly = false, dragging = true))
            }
        }
    }

    companion object {
        // 手柄拖动期间两次原生推送之间的最小间隔：一帧（60Hz），即高亮最多落后
        // 手指一帧。更长（原先 50ms≈3 帧）在快速拖动时可被看出「不跟手」，
        // 更短不会更快——notifyRender 会立刻让渲染线程做一次整屏重绘，
        // 这个间隔正是拖动期间的渲染节奏上限。
        private const val DRAG_NATIVE_SYNC_INTERVAL_MS = 16L
    }
}
