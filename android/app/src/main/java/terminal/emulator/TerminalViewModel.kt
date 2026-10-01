package terminal.emulator

import android.content.Context
import android.content.Intent
import android.os.SystemClock
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.Surface
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import terminal.emulator.bridge.FontInfoDto
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.KeyboardMode
import terminal.emulator.input.ModifierState
import terminal.emulator.input.toggled
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.PasteChunker
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.clampSelection
import terminal.emulator.util.TerminalDispatchers
import terminal.emulator.util.runCatchingCancellable
import javax.inject.Inject

private const val CLIPBOARD_TEXT_MAX_LENGTH = 100_000

/** Mirror of selection::TouchClass. */
enum class TouchClass {
    Text,
    Whitespace,
    EmptyArea,
    Unknown,
}

/** 上游选择派生查询回传的界限数组长度：[startRow, startCol, endRow, endCol]。 */
internal const val SELECTION_BOUNDS_LENGTH = 4

data class SelectionAnchor(val row: Int, val col: Int)

data class SelectionState(
    val active: Boolean = false,
    val dragging: Boolean = false,
    val start: SelectionAnchor? = null,
    val end: SelectionAnchor? = null,
    val selectedText: String = "",
    val touchClass: TouchClass = TouchClass.Unknown,
    // 已执行菜单动作（复制/全选/分享）时置位：浮动菜单关闭而选区高亮保留。
    // 下次长按或拖动手柄移动时重置。
    val menuDismissed: Boolean = false,
) {
    val pasteOnly: Boolean
        get() = touchClass == TouchClass.EmptyArea || touchClass == TouchClass.Whitespace

    val hasSelection: Boolean
        get() = active && start != null && end != null

    fun applyHandleDrag(draggingStart: Boolean, targetRow: Int, targetCol: Int): HandleDragResult {
        val currentEnd = end ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol)
        val currentStart = start ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol)
        if (
            draggingStart &&
            (
                targetRow > currentEnd.row ||
                    (targetRow == currentEnd.row && targetCol >= currentEnd.col)
                )
        ) {
            return HandleDragResult(currentEnd.row, currentEnd.col, targetRow, targetCol)
        }
        if (
            !draggingStart &&
            (
                targetRow < currentStart.row ||
                    (targetRow == currentStart.row && targetCol <= currentStart.col)
                )
        ) {
            return HandleDragResult(targetRow, targetCol, currentStart.row, currentStart.col)
        }
        if (draggingStart) {
            return HandleDragResult(targetRow, targetCol, currentEnd.row, currentEnd.col)
        }
        return HandleDragResult(currentStart.row, currentStart.col, targetRow, targetCol)
    }
}

data class HandleDragResult(val startRow: Int, val startCol: Int, val endRow: Int, val endCol: Int)

/** 会话抽屉条目：序号按列表位置渲染，[title] 为该会话终端标题（OSC 0/2）。 */
data class SessionInfo(val id: Long, val title: String = "")

/** 非强制会话元数据刷新节流窗口；抽屉打开、关闭会话的强制刷新不受此限。 */
internal const val SESSION_META_REFRESH_THROTTLE_MS = 2000L

data class TerminalState(
    val sessionId: Long = 0L,
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val selection: SelectionState = SelectionState(),
    val ctrlState: ModifierState = ModifierState.Off,
    val altState: ModifierState = ModifierState.Off,
    val scrollActive: Boolean = false,
    val sessions: List<SessionInfo> = emptyList(),
    val activeSessionId: Long = 0L,

    /** IME 编辑器类型固定为 Secure（DESIGN.md 要求全功能输入法，不提供切换入口）。 */
    val keyboardMode: KeyboardMode = KeyboardMode.Secure,
    val selectionAccent: Int = 0,
    // 每次程序化滚动复位（由输入驱动的贴底）时递增；TerminalScreen 观察它
    // 并重同步 Surface 的本地选区计算偏移。
    val scrollEpoch: Long = 0L,
)

/**
 * 输入驱动的滚动复位决策，纯函数以便表驱动测试：
 * 含换行/回车的用户输入表明了与实时屏幕交互的意图，故视口立即贴回
 * ——不等 shell 回显（在空闲提示符上单独按回车根本不产生任何输出，
 * 这正是所反馈的现象）。
 *
 * @param hasSelectionOrDrag 选区激活或手柄拖拽会像输出驱动路径一样抑制贴底。
 */
internal fun shouldResetScrollOnInput(data: ByteArray, hasSelectionOrDrag: Boolean): Boolean = !hasSelectionOrDrag &&
    data.any { byte -> byte == '\r'.code.toByte() || byte == '\n'.code.toByte() }

internal fun shouldCreateDefaultSession(
    surfaceValid: Boolean,
    surfaceWidth: Int,
    surfaceHeight: Int,
    uiSessions: List<SessionInfo>,
    runtimeSessionIds: List<Long>,
): Boolean = surfaceValid &&
    surfaceWidth > 0 &&
    surfaceHeight > 0 &&
    uiSessions.isEmpty() &&
    runtimeSessionIds.isEmpty()

// ═══════════════════════════════════════════════════════════════════════════
// 一、字段与构造函数
// ═══════════════════════════════════════════════════════════════════════════

@HiltViewModel
class TerminalViewModel
@Inject
constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepository: SettingsRepository,
    val runtime: TerminalRuntime,
) : ViewModel() {
    private val clipboardAccess = ClipboardAccess(context, tag = "ViewModel")

    private val selectionManager = SelectionManager()

    // 从运行期见到的最近网格尺寸；缩小时会钳位活跃选区（见 runtime.state 收集器）。
    @Volatile private var lastGridRows = 0

    @Volatile private var lastGridCols = 0

    // 视图侧回写的回滚长度缓存：选区行是绝对行（0 = 回滚顶部），
    // 钳位上界必须是绝对空间最后一行（回滚长度 + 视口行数 - 1），不得用视口行数。
    @Volatile private var cachedScrollbackLength = 0

    /** 视图侧每次刷新回滚长度后回写，使选区钳位与视图用同一口径。 */
    internal fun updateScrollbackLength(length: Int) {
        cachedScrollbackLength = length
    }

    /** 绝对空间的最后一行：两处选区钳位的唯一来源，避免各自计算再次漂移。 */
    private fun absoluteMaxRow(rows: Int): Int = (cachedScrollbackLength + rows - 1).coerceAtLeast(0)
    private val fontManager = FontManager()

    // ── 字体转发（实现在 FontManager） ──

    fun setFontSize(size: Float) = fontManager.setFontSize(size)

    /** 拖动预览（不写设置、不重排网格）——见 FontManager。 */
    fun setFontSizeInPlacePreview(size: Float) = fontManager.setFontSizeInPlacePreview(size)

    fun setFontFamily(family: String) = fontManager.setFontFamily(family)

    // ── 选区转发（实现在 SelectionManager） ──

    fun startSelection(row: Int, col: Int, touchClass: TouchClass = TouchClass.Unknown) =
        selectionManager.startSelection(row, col, touchClass)

    fun updateSelection(row: Int, col: Int) = selectionManager.updateSelection(row, col)

    fun updateSelectionStart(row: Int, col: Int) = selectionManager.updateSelectionStart(row, col)

    fun endSelection() = selectionManager.endSelection()

    /** 快速拖动路径：计算选区边界而不写 Compose 状态；见 [SelectionManager.dragMove]。 */
    fun dragMove(draggingStart: Boolean, row: Int, col: Int, cachedMaxRow: Int, cachedMaxCol: Int): IntArray? =
        selectionManager.dragMove(draggingStart, row, col, cachedMaxRow, cachedMaxCol)

    /** 把最后一次快速路径的拖动边界提交到 Compose 状态；见 [SelectionManager.commitDragBounds]。 */
    fun commitDragBounds() = selectionManager.commitDragBounds()

    /**
     * 边缘滚动手柄拖动期间以节流频率把当前 Compose 选区推送到原生：
     * 慢速路径（[updateSelection]/[updateSelectionStart]）已写入状态，
     * 故读回并共用快速路径的节奏守卫，使两条拖动路径的高亮速率一致。
     */
    fun syncDragSelectionToNativeThrottled() {
        val current = _state.value.selection
        val start = current.start ?: return
        val end = current.end ?: return
        if (!current.active || current.pasteOnly) return
        selectionManager.syncDragBoundsToNativeThrottled(
            intArrayOf(start.row, start.col, end.row, end.col),
        )
    }

    /**
     * termux setInitialTextSelectionPosition 流程：在空白单元格/仅粘贴的选区上抓住手柄时，
     * 将其升级为普通文本选区，使随后的拖动能*扩展*范围，
     * 而不被「仅粘贴不可变」守卫吞掉。
     * 该守卫仍保护长按的微小移动窗口——转换仅在刻意锁定手柄时发生。
     */
    fun beginHandleDragOnPasteOnly() {
        _state.update { state ->
            val current = state.selection
            if (!current.active || !current.pasteOnly) {
                state
            } else {
                // dragging=true 与升级同批次：Compose 侧 showSelectionHandles
                // 在 dragging 下跳过重建——否则升级引发的重组会重建 overlay
                // 弹窗，杀死正进行的手势流（柄刚抓住就松手，拖拽永不生长）。
                // endSelection（松手）负责清回 false，生命周期闭环。
                state.copy(selection = current.copy(touchClass = TouchClass.Text, dragging = true))
            }
        }
    }

    fun copySelectionToClipboard() = selectionManager.copySelectionToClipboard()

    fun clearSelection() = selectionManager.clearSelection()

    fun showPastePopup(row: Int, col: Int) = selectionManager.showPastePopup(row, col)

    fun shareSelection() = selectionManager.shareSelection()

    fun selectAll() = selectionManager.selectAll()

    fun pasteFromClipboard(): Int = selectionManager.pasteFromClipboard()

    /** 网格 resize 后把选区锚点钳位到绝对网格，使原生 setSelection 绝不收到越界单元格。 */
    private fun clampSelectionToGrid(selection: SelectionState, rows: Int, cols: Int): SelectionState {
        val start = selection.start ?: return selection
        val end = selection.end ?: return selection
        val maxRow = absoluteMaxRow(rows)
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
     * 共用的输入驱动滚动处理：任何用户输入都清除 SCROLL 锁；
     * 提交性输入（CR/LF 或硬件回车）还会把视口立即贴到实时屏幕，不等 PTY 输出。
     * 抽出此方法使输入法 [writeToPty] 路径与硬件 [terminal.emulator.ui.TerminalSurface.onKeyDown] 路径共用一个贴底点
     * ——硬件回车经 bridge.processKeyEvent 绕过 [writeToPty]，
     * 正是「输入新命令 + 回车不滚动」这一反馈的根因。
     */
    internal fun onUserInputForScrollSnap(isCommit: Boolean) {
        if (_state.value.scrollActive) {
            _state.update { it.copy(scrollActive = false) }
            runtime.setScrollActive(false)
        }
        if (!isCommit) return
        val selection = _state.value.selection
        if (selection.active || selection.dragging) return
        runtime.setScrollOffset(0)
        _state.update { it.copy(scrollEpoch = it.scrollEpoch + 1) }
    }

    fun writeToPty(data: ByteArray) {
        val isCommit =
            shouldResetScrollOnInput(data, _state.value.selection.let { it.active || it.dragging })
        // hasSelectionOrDrag 在 onUserInputForScrollSnap 内部也会检查；
        // 此处透传，使共用的辅助函数拥有最终守卫权。
        onUserInputForScrollSnap(isCommit)
        val written = runtime.writeToPty(data)
        if (!written) {
            LogUtil.e("TerminalViewModel", "writeToPty failed for ${data.size} bytes")
        }
    }

    /** Feed bytes directly to the VT parser (test path for escape sequences). */
    fun feedTerminal(data: ByteArray) {
        runtime.feedTerminal(data)
    }

    fun cycleCtrlState() {
        _state.update { it.copy(ctrlState = it.ctrlState.toggled()) }
    }

    fun cycleAltState() {
        _state.update { it.copy(altState = it.altState.toggled()) }
    }

    fun lockCtrlState() {
        _state.update { it.copy(ctrlState = ModifierState.Locked) }
    }

    fun lockAltState() {
        _state.update { it.copy(altState = ModifierState.Locked) }
    }

    fun consumeOneShotModifiers() {
        val currentState = _state.value
        var newCtrl = currentState.ctrlState
        var newAlt = currentState.altState
        if (newCtrl == ModifierState.Once) newCtrl = ModifierState.Off
        if (newAlt == ModifierState.Once) newAlt = ModifierState.Off
        if (newCtrl != currentState.ctrlState || newAlt != currentState.altState) {
            _state.update { current -> current.copy(ctrlState = newCtrl, altState = newAlt) }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3b: 选区管理
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有选区状态转换：开始/更新/结束、剪贴板复制/分享、全选，
     * 以及提取启发式（URL 拼接、TUI 边框检测）。内部类：
     * 经外层 ViewModel 访问 _state、runtime 与 clipboardPaster。
     */
    inner class SelectionManager {
        fun startSelection(row: Int, col: Int, touchClass: TouchClass = TouchClass.Unknown) {
            val anchor = SelectionAnchor(row, col)
            // CAS：选区在主线程上被触碰，但 _state 也被 IO 协程写入；
            // 普通读-改-写可能丢失其中之一的更新。mode 在 lambda 内读取。
            _state.update { state ->
                state.copy(
                    selection =
                    SelectionState(
                        active = true,
                        dragging = true,
                        start = anchor,
                        end = anchor,
                        touchClass = touchClass,
                        menuDismissed = false,
                    ),
                )
            }
        }

        fun updateSelection(row: Int, col: Int) {
            dragSelection(draggingStart = false, row = row, col = col)
        }

        fun updateSelectionStart(row: Int, col: Int) {
            dragSelection(draggingStart = true, row = row, col = col)
        }

        /**
         * 快速拖动路径：不触碰 Compose 状态地计算选区边界。
         * 返回 intArrayOf(startRow, startCol, endRow, endCol) 供直接重定位手柄；
         * 拖动未激活或仅粘贴时返回 null。
         *
         * 调用方直接依返回的边界重定位手柄，避开 Compose _state
         * → 重组 → 回读的往返，而这正是每次 MOVE 的主要帧瓶颈。
         */
        fun dragMove(draggingStart: Boolean, row: Int, col: Int, cachedMaxRow: Int, cachedMaxCol: Int): IntArray? {
            val current = _state.value.selection
            if (!current.active || current.pasteOnly) return null
            val result =
                current.applyHandleDrag(
                    draggingStart = draggingStart,
                    targetRow = row,
                    targetCol = col,
                )
            val bounds =
                clampSelection(
                    result.startRow,
                    result.startCol,
                    result.endRow,
                    result.endCol,
                    cachedMaxRow,
                    cachedMaxCol,
                )
            val boundsArray = intArrayOf(bounds.startRow, bounds.startCol, bounds.endRow, bounds.endCol)
            lastDragBounds = boundsArray
            syncDragBoundsToNativeThrottled(boundsArray)
            return boundsArray
        }

        /**
         * 以节流频率把进行中的拖动边界推送到原生渲染器，使单元格反色高亮实时跟随手柄
         * （termux updatePosition 对等），而无需在每个 MOVE 帧都付出一次 JNI 穿越与重渲染。
         * 最终的精确边界由松手时 endSelection 经 runtime.setSelection 提交。
         */
        internal fun syncDragBoundsToNativeThrottled(boundsArray: IntArray) {
            val nowMs = SystemClock.uptimeMillis()
            if (nowMs - lastDragNativeSyncUptimeMs < DRAG_NATIVE_SYNC_INTERVAL_MS) return
            lastDragNativeSyncUptimeMs = nowMs
            val loRow = minOf(boundsArray[0], boundsArray[2])
            val hiRow = maxOf(boundsArray[0], boundsArray[2])
            val loCol = minOf(boundsArray[1], boundsArray[3])
            val hiCol = maxOf(boundsArray[1], boundsArray[3])
            runtime.setSelection(loRow, loCol, hiRow, hiCol, true)
        }

        @Volatile private var lastDragNativeSyncUptimeMs = 0L

        /**
         * 在 endSelection 之前把最后一次 dragMove 的边界提交到 Compose 状态。
         * 必须由 finishHandleDrag 调用，使 endSelection 读到正确的边界。
         */
        fun commitDragBounds() {
            val boundsArray = lastDragBounds ?: return
            _state.update { state ->
                val cur = state.selection
                if (!cur.active) return@update state
                state.copy(
                    selection =
                    cur.copy(
                        start = SelectionAnchor(boundsArray[0], boundsArray[1]),
                        end = SelectionAnchor(boundsArray[2], boundsArray[3]),
                    ),
                )
            }
            lastDragBounds = null
        }

        private var lastDragBounds: IntArray? = null

        private fun dragSelection(draggingStart: Boolean, row: Int, col: Int) {
            // 拖动路径钳位所用的绝对网格边界（见下）；只读一次——
            // 并发的 resize 最多使该边界陈旧一帧。
            val runtimeState = runtime.state.value
            val maxRow = absoluteMaxRow(runtimeState.rows)
            val maxCol = (runtimeState.cols - 1).coerceAtLeast(0)
            // CAS，活跃性检查置于 lambda 内部：选区读取与写入相对并发的
            // _state 更新是原子的。仅粘贴的选区（空单元格/空白处长按）不可变
            // ——长按期间手指的微小移动绝不能使该单格漂移。
            _state.update { state ->
                val current = state.selection
                if (!current.active || current.pasteOnly) {
                    state
                } else {
                    val result =
                        current.applyHandleDrag(
                            draggingStart = draggingStart,
                            targetRow = row,
                            targetCol = col,
                        )
                    // 纵深防御：在拖动结果抵达原生 setSelection 之前先规范化
                    // ——保序的 start ≤ end + 范围钳位到网格。
                    // applyHandleDrag 本身已正确排序；clampSelection
                    // 是倒置/越界目标的安全网。
                    val bounds =
                        clampSelection(
                            result.startRow,
                            result.startCol,
                            result.endRow,
                            result.endCol,
                            maxRow,
                            maxCol,
                        )
                    state.copy(
                        selection =
                        current.copy(
                            dragging = true,
                            start = SelectionAnchor(bounds.startRow, bounds.startCol),
                            end = SelectionAnchor(bounds.endRow, bounds.endCol),
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
            if (_state.value.selection.dragging) {
                runtime.setSelectionDragging(true)
            }
        }

        fun endSelection() {
            val current = _state.value.selection
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
                val state = _state.value
                if (state.selection != current) return
                if (_state.compareAndSet(state, state.copy(selection = updated))) break
            }
            val start = current.start
            val end = current.end
            val loRow = minOf(start.row, end.row)
            val hiRow = maxOf(start.row, end.row)
            val loCol = minOf(start.col, end.col)
            val hiCol = maxOf(start.col, end.col)
            runtime.setSelection(loRow, loCol, hiRow, hiCol, true)
        }

        fun copySelectionToClipboard() {
            val selection = _state.value.selection
            // 重新提取而不信任缓存的 selectedText：若缓存文本为空
            // （某些回滚状态下原生 selection_text 为 null），
            // 会出现「复制按钮毫无反应」的反馈。
            // 按需提取保证菜单动作总是复制当前选区。
            val rawText =
                if (selection.selectedText.isNotEmpty()) {
                    selection.selectedText
                } else {
                    extractSelectedText(selection)
                }
            if (rawText.isEmpty()) return
            val clipped =
                if (rawText.length > CLIPBOARD_TEXT_MAX_LENGTH) {
                    rawText.substring(0, CLIPBOARD_TEXT_MAX_LENGTH)
                } else {
                    rawText
                }
            clipboardAccess.setClipboardText(clipped, label = "terminal selection")
            // 动作完成后关闭浮动菜单；保留高亮。
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }

        fun clearSelection() {
            _state.update { it.copy(selection = SelectionState()) }
            syncSelectionToNative()
            // 强制立即重绘：否则原生侧的脏标志要等下一个 vsync 节拍，
            // 使陈旧的选区高亮在屏幕上残留长达一帧周期。
            runtime.forceRender()
        }

        fun showPastePopup(row: Int, col: Int) {
            // 空单元格处长按现在会创建一个单格选区
            // （经 GPU 路径反色背景）并配仅粘贴的浮动菜单
            // ——与文本选区的交互一致，而非无高亮的孤立小片。
            _state.update { state ->
                state.copy(
                    selection =
                    SelectionState(
                        active = true,
                        dragging = false,
                        start = SelectionAnchor(row, col),
                        end = SelectionAnchor(row, col),
                        touchClass = TouchClass.EmptyArea,
                    ),
                )
            }
            syncSelectionToNative()
        }

        private fun syncSelectionToNative() {
            val selection = _state.value.selection
            if (selection.active && selection.start != null && selection.end != null) {
                val start = selection.start
                val end = selection.end
                val loRow = minOf(start.row, end.row)
                val hiRow = maxOf(start.row, end.row)
                val loCol = minOf(start.col, end.col)
                val hiCol = maxOf(start.col, end.col)
                runtime.setSelection(loRow, loCol, hiRow, hiCol, true)
            } else {
                runtime.setSelection(0, 0, 0, 0, false)
            }
        }

        fun shareSelection() {
            val rawText = _state.value.selection.selectedText
            if (rawText.isEmpty()) return
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
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }

        fun selectAll() {
            // 上游 select_all：整个内容（回滚 + 视口）一次派生，native 侧已
            // 安装为终端选区，Kotlin 只消费回传的有序界限（不含尾部空行）。
            val bounds = runtime.bridge()?.selectAll()
            if (bounds == null || bounds.size != SELECTION_BOUNDS_LENGTH) {
                // 无内容/查询失败：按“无数据”处理，不伪造选择。
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
            _state.update { it.copy(selection = selectionState.copy(selectedText = text)) }
            syncSelectionToNative()
        }

        private fun extractSelectedText(selection: SelectionState): String {
            val start = selection.start ?: return ""
            val end = selection.end ?: return ""
            val bridge = runtime.bridge() ?: return ""
            // 选区行以网格坐标存储（0 = 回滚顶部），与 Ghostty 格式化器的网格行一致。
            //
            // 换行感知的提取（termux TerminalBuffer.getSelectedText 语义）：
            // 软换行行拼接时不插入
            // 软换行处不插入 '\n'（解包）并去除尾部空白，
            // 且格式化器在内部把网格列映射到字符下标，使 CJK 宽字符绝不被切开
            // （等效于 TerminalRow.findStartOfColumn）。
            // 旧的逐行 scrollbackLine + '\n' 拼接无法侦测换行，且可能切开代理对。
            val (lo, hi) =
                if (start.row < end.row || (start.row == end.row && start.col <= end.col)) {
                    start to end
                } else {
                    end to start
                }
            return bridge.selectionText(lo.row, lo.col, hi.row, hi.col) ?: ""
        }

        /** Paste clipboard content directly to the PTY (no confirmation dialog). */
        fun pasteFromClipboard(): Int {
            val text = clipboardAccess.clipboardText() ?: return 0
            return executePaste(text)
        }

        /** Actually send [text] to PTY via the chunker. */
        fun executePaste(text: String): Int {
            var offset = 0
            for (chunk in PasteChunker().chunks(text)) {
                runtime.writeToPty(chunk.toByteArray())
                offset += chunk.length
            }
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
            return offset
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 二之二、字体管理
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有字体加载、字号/家族设置与字体文件安装。内部类：
     * 经外层 ViewModel 访问字体 StateFlow、runtime、settingsRepository 与 context。
     * loadFonts() 在安装后刷新这些流。
     */
    inner class FontManager {
        /**
         * DESIGN 字体选择节：native 应用返回 false 即设置数据错误，
         * 输出日志并清除该设置使会话回落默认。null 表示无会话可验证，不处理。
         */
        private suspend fun clearUnknownFontFamily(requestedFamily: String, applied: Boolean?) {
            if (applied != false) return
            LogUtil.e("Font", "Unknown font family, clearing setting: $requestedFamily")
            settingsRepository.clearFontFamily()
        }

        fun loadFonts() {
            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                try {
                    val bridge = runtime.bridge()
                    // 先注册用户投放目录再列举，使其字体家族即使在 loadFonts
                    // 早于 Runtime.start() 完成时也被纳入（重复注册为空操作）。
                    terminal.emulator.termuxFontDir(context).takeIf { it.isDirectory }?.let { dir ->
                        bridge?.setExtraFontPaths(listOf(dir.absolutePath))
                    }
                    val allFonts =
                        terminal.emulator.settings.availableFontFamilies(
                            bridge?.listFontFamilies().orEmpty(),
                        )
                    _availableFonts.value = allFonts
                    val defaultName = bridge?.getDefaultFontName().orEmpty()
                    _defaultFontName.value = defaultName.ifEmpty { allFonts.first() }
                    val storedFamily = settingsRepository.fontFamily.first()
                    clearUnknownFontFamily(
                        storedFamily,
                        bridge?.setFontFamily(
                            terminal.emulator.resolveEffectiveFontFamily(storedFamily),
                        ),
                    )
                    _fontInfo.value =
                        bridge?.getFontInfo() ?: FontInfoDto.placeholderJson(_defaultFontName.value)
                } catch (fatal: IllegalStateException) {
                    if (fatal is kotlinx.coroutines.CancellationException) throw fatal
                    // 系统 fonts.xml 缺失或不可解析：按 DESIGN 记录日志并崩溃退出，
                    // 不得静默回退为空列表。
                    LogUtil.e("TerminalViewModel", "Fatal: system fonts.xml unreadable", fatal)
                    throw fatal
                } catch (exception: Exception) {
                    if (exception is kotlinx.coroutines.CancellationException) throw exception
                    LogUtil.e("TerminalViewModel", "Failed to load font list", exception)
                    throw exception
                }
            }
        }

        // 串行化完整的字体应用链（DataStore 写入 + bridge 字体重载 + 网格重排）。
        // 没有它，快速拖动滑块会启动并发 IO 协程，
        // 使原生 setFontSizeInPlace 调用乱序交错
        // （实测一次拖动中值从 96 跳到 280）并在序列中途重排网格
        // ——即布局错乱的反馈来源。
        private val fontApplyMutex = Mutex()

        /**
         * 轻量的拖动预览：把字号应用到原生管线并刷新单元格度量，
         * 但不写设置、不重排网格。开销足够低，可每个拖动步都运行；
         * 松手手势经 [setFontSize] 提交。
         */
        fun setFontSizeInPlacePreview(size: Float) {
            runtime.setFontSizePreview(size)
        }

        fun setFontSize(size: Float) {
            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                fontApplyMutex.withLock {
                    settingsRepository.setFontSize(size)
                    runtime.applyFontSettings()
                    val bridge = runtime.bridge()
                    if (bridge != null) {
                        _fontInfo.value = bridge.getFontInfo() ?: context.getString(R.string.no_font_loaded)
                    }
                }
            }
        }

        fun setFontFamily(family: String) {
            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                try {
                    LogUtil.d("Font", "Setting font family: $family")
                    settingsRepository.setFontFamily(family)
                    val applied = runtime.applyFontSettings()
                    val bridge = runtime.bridge()
                    val fontName = bridge?.getDefaultFontName()
                    clearUnknownFontFamily(family, applied)
                    val fontInfo = bridge?.getFontInfo() ?: context.getString(R.string.no_font_loaded)
                    _defaultFontName.value = fontName ?: ""
                    _fontInfo.value = fontInfo
                    LogUtil.d("Font", "Font applied: ${_defaultFontName.value}")
                    kotlinx.coroutines.withContext(TerminalDispatchers.main) {
                        val message =
                            if (applied == false) {
                                context.getString(R.string.font_apply_failed, family)
                            } else {
                                context.getString(R.string.font_applied, _defaultFontName.value)
                            }
                        android.widget.Toast.makeText(
                            context,
                            message,
                            android.widget.Toast.LENGTH_SHORT,
                        )
                            .show()
                    }
                } catch (exception: Exception) {
                    LogUtil.e("Font", "setFontFamily failed for $family", exception)
                    kotlinx.coroutines.withContext(TerminalDispatchers.main) {
                        android.widget.Toast.makeText(
                            context,
                            context.getString(R.string.font_apply_failed, exception.message ?: ""),
                            android.widget.Toast.LENGTH_SHORT,
                        )
                            .show()
                    }
                }
            }
        }
    }

    companion object {
        // 手柄拖动期间两次原生 setSelection 推送之间的最小间隔：
        // 实时高亮节奏（termux 对等）与逐帧 JNI + 重渲染开销之间的取舍。
        private const val DRAG_NATIVE_SYNC_INTERVAL_MS = 50L
        private const val TAG = "TerminalViewModel"
        private const val TIMEOUT_MILLIS = 5_000L
        private const val DEBOUNCE_MILLIS = 300L

        // 剪贴板粘贴的上界（主线程字符串拷贝），也是流式发送它所用的块大小
        // （必须远低于 PTY 缓冲）。
    }

    private val _state = MutableStateFlow(TerminalState())
    val state: StateFlow<TerminalState> = _state.asStateFlow()

    @Volatile var currentSurface: Surface? = null

    @Volatile var surfaceWidth: Int = 0

    @Volatile var surfaceHeight: Int = 0

    fun startRuntime(surface: Surface?, width: Int, height: Int) {
        currentSurface = surface
        surfaceWidth = width
        surfaceHeight = height
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            runtime.start(surface, width, height)
        }
    }

    /**
     * 全部持久化设置的单一合并快照。UI 订阅这一条 StateFlow；
     * 按字段访问形如 `settings.fontSize`。
     */
    val settings: StateFlow<SettingsRepository.SettingsState> =
        settingsRepository.settings.stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(TIMEOUT_MILLIS),
            // 与按设备自适应的默认值一致，使流建立前的快照
            // 不会闪现固定的 10sp 兜底值。
            SettingsRepository.SettingsState(
                fontSize =
                SettingsRepository.defaultFontSizeFor(
                    context.resources.displayMetrics.widthPixels /
                        context.resources.displayMetrics.density,
                ),
            ),
        )

    private val _availableFonts = MutableStateFlow<List<String>>(emptyList())
    val availableFonts: StateFlow<List<String>> = _availableFonts.asStateFlow()

    private val _defaultFontName = MutableStateFlow("")
    val defaultFontName: StateFlow<String> = _defaultFontName.asStateFlow()

    private val _fontInfo = MutableStateFlow(FontInfoDto.placeholderJson(""))
    val fontInfo: StateFlow<String> = _fontInfo.asStateFlow()

    init {
        // 首次启动：固定一个按设备自适应的默认字号，使网格在用户触碰字号滑块前即可读。
        viewModelScope.launch {
            val metrics = context.resources.displayMetrics
            settingsRepository.applyFirstLaunchDefaultFontSize(metrics.widthPixels / metrics.density)
        }
        viewModelScope.launch {
            runtime.state.collect { runtimeState ->
                // 网格 resize 可能把尺寸缩到当前选区边界之下；
                // 把 start/end 钳位到新网格，使原生 setSelection 绝不看到越界单元格。
                if (runtimeState.rows != lastGridRows || runtimeState.cols != lastGridCols) {
                    lastGridRows = runtimeState.rows
                    lastGridCols = runtimeState.cols
                    _state.update { current ->
                        current.copy(
                            selection =
                            clampSelectionToGrid(current.selection, runtimeState.rows, runtimeState.cols),
                        )
                    }
                }
                val sortedIds = runtimeState.sessionIds.sorted()
                val previousById = _state.value.sessions.associateBy { it.id }
                val sessions = sortedIds.map { id -> previousById[id] ?: SessionInfo(id = id) }
                val active = runtimeState.activeSessionId
                if (active != 0L) {
                    val displayIndex = sortedIds.indexOf(active) + 1
                    val title =
                        if (runtimeState.title.isNotEmpty()) {
                            runtimeState.title
                        } else {
                            context.getString(R.string.session_number, displayIndex)
                        }
                    // 用 _state.update（CAS）而非读-改-写：
                    // IO 调度器上的 createSession/switchSession 也会更新 _state，
                    // 此处非原子的写入会覆盖它们刚提交的会话列表。
                    _state.update { current ->
                        current.copy(
                            sessionId = active,
                            isRunning = runtimeState.isRunning,
                            title = title,
                            sessions = sessions,
                            activeSessionId = active,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                    if (runtime.state.value.sessionIds.isNotEmpty()) {
                        if (_availableFonts.value.isEmpty()) {
                            fontManager.loadFonts()
                        } else {
                            // 字体查询是同步 JNI，不得在 Main.immediate 收集器上执行。
                            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                                val bridge = runtime.bridge()
                                _defaultFontName.value = bridge?.getDefaultFontName() ?: ""
                                _fontInfo.value =
                                    bridge?.getFontInfo() ?: context.getString(R.string.no_font_loaded)
                            }
                        }
                    }
                } else {
                    _state.update { current ->
                        current.copy(sessions = sessions, activeSessionId = active)
                    }
                }
                refreshSessionMetas()
            }
        }
    }

    /**
     * 删除与用户数据（files 目录下的 home/usr）无关的设置、崩溃循环状态与缓存数据，
     * 并重建 DataStore 的 prefs 目录，使下一次设置写入不会失败。
     *
     * [onComplete] 在 IO 调度器上回调（非主线程）；界面工作（如 Toast）需自行切回主线程
     * 或使用 Android 自动投递的 Toast API。删除失败按 DESIGN 错误策略抛出，不回调成功。
     */
    fun clearAppData(onComplete: () -> Unit) {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            context.getDir("prefs", Context.MODE_PRIVATE).deleteRecursively()
            context.getDir("boot_state", Context.MODE_PRIVATE).deleteRecursively()
            context.cacheDir.listFiles()?.forEach { it.deleteRecursively() }
            // 进程级 DataStore 单例仍在运行：重建 prefs 目录使下一次设置写入不会失败。
            context.getDir("prefs", Context.MODE_PRIVATE)
            onComplete()
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 二、会话编排与设置 setter
    // ══════════════════════════════════════════════════════════════════════

    /** 非强制会话元数据刷新节流窗口；抽屉打开、关闭会话的强制刷新不受此限。 */
    private var lastMetaRefreshMs: Long = 0L

    /**
     * 回填抽屉列表的终端标题（序号由列表按位置从 1 递增渲染）。 JNI 查询在 IO 线程执行；
     * 写入时校验集合未变，避免覆盖更新的列表。
     */
    fun refreshSessionMetas(force: Boolean = false) {
        if (!force && SystemClock.uptimeMillis() - lastMetaRefreshMs < SESSION_META_REFRESH_THROTTLE_MS) return
        lastMetaRefreshMs = SystemClock.uptimeMillis()
        val ids = _state.value.sessions.map { it.id }.sorted()
        if (ids.isEmpty()) return
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            val fresh = ids.map { id ->
                val title = runCatchingCancellable { NativeBridge.getTitle(id) }.getOrNull()
                SessionInfo(id = id, title = title.orEmpty())
            }
            withContext(TerminalDispatchers.main) {
                _state.update { current ->
                    if (current.sessions.map { it.id }.sorted() != ids) {
                        current
                    } else {
                        current.copy(sessions = fresh)
                    }
                }
            }
        }
    }

    fun ensureDefaultSession() {
        if (
            !shouldCreateDefaultSession(
                surfaceValid = currentSurface?.isValid == true,
                surfaceWidth = surfaceWidth,
                surfaceHeight = surfaceHeight,
                uiSessions = _state.value.sessions,
                runtimeSessionIds = runtime.state.value.sessionIds,
            )
        ) {
            return
        }
        LogUtil.d("TerminalViewModel", "ensureDefaultSession: creating default session")
        createSession()
    }

    fun setBootstrapUrl(url: String) {
        bootstrapUrlEdited = true
        bootstrapUrlEdits.tryEmit(url)
    }

    private val _bootstrapRunning = MutableStateFlow(false)
    val bootstrapRunning: StateFlow<Boolean> = _bootstrapRunning.asStateFlow()

    private val _bootstrapResult = MutableStateFlow<String?>(null)
    val bootstrapResult: StateFlow<String?> = _bootstrapResult.asStateFlow()

    private val _bootstrapProgress =
        MutableStateFlow<terminal.emulator.installer.BootstrapProgress?>(null)
    val bootstrapProgress: StateFlow<terminal.emulator.installer.BootstrapProgress?> =
        _bootstrapProgress.asStateFlow()

    /** 把引导结果映射为本地化文案；失败负载是机器可读键（见 [terminal.emulator.installer.BootstrapOrchestrator]）。 */
    private fun bootstrapOutcomeText(result: Result<String>): String = result.fold(
        onSuccess = { diagnostics ->
            val headline = context.getString(R.string.bootstrap_installed_success)
            if (diagnostics.isEmpty()) {
                headline
            } else {
                context.getString(R.string.bootstrap_postinst_errors) + "\n" + diagnostics
            }
        },
        onFailure = { exception ->
            when (exception.message) {
                terminal.emulator.installer.BootstrapOrchestrator.ERROR_PRIMARY_USER_REQUIRED ->
                    context.getString(R.string.bootstrap_primary_user_required)

                terminal.emulator.installer.BootstrapOrchestrator.ERROR_ALREADY_IN_PROGRESS ->
                    context.getString(R.string.bootstrap_already_in_progress)

                terminal.emulator.installer.BootstrapOrchestrator.ERROR_NO_URL ->
                    context.getString(R.string.bootstrap_no_url)

                terminal.emulator.installer.BootstrapOrchestrator.ERROR_CANCELLED ->
                    context.getString(R.string.bootstrap_cancelled)

                else -> context.getString(R.string.bootstrap_error, exception.javaClass.simpleName)
            }
        },
    )

    /**
     * 在线与离线安装路径共用的 [terminal.emulator.installer.BootstrapInstaller] 与 [terminal.emulator.installer.SecondStageRunner] 配对。
     * 文件位于 `filesDir` 之下，故操作系统只能经应用数据管理回收它们，绝不会因缓存压力而清理。
     */
    private fun bootstrapComponents(
        onProgress: terminal.emulator.installer.BootstrapProgressCallback,
    ): Pair<
        terminal.emulator.installer.BootstrapInstaller,
        terminal.emulator.installer.SecondStageRunner,
        > {
        val installer =
            terminal.emulator.installer.BootstrapInstaller(
                prefixDir = java.io.File(context.filesDir, "usr"),
                homeDir = java.io.File(context.filesDir, "home"),
                stagingDir = java.io.File(context.filesDir, "usr-staging"),
                onProgress = onProgress,
            )
        val secondStage =
            terminal.emulator.installer.SecondStageRunner(
                prefixDir = java.io.File(context.filesDir, "usr"),
                homeDir = java.io.File(context.filesDir, "home"),
                onProgress = onProgress,
            )
        return installer to secondStage
    }

    /**
     * 引导安装共用的守卫与协程骨架：经 [bootstrapRunning] 上的 CAS 串行化并发运行、
     * 重置进度状态，并把失败映射为同一条错误消息。
     * 调用方提供安装主体，它会收到共享的进度回调。
     */
    private fun startBootstrapJob(block: suspend (terminal.emulator.installer.BootstrapProgressCallback) -> Unit) {
        // CAS 使快速连点安装按钮无法启动两个并发安装。
        if (!_bootstrapRunning.compareAndSet(false, true)) return
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            _bootstrapResult.value = null
            _bootstrapProgress.value = null
            try {
                val onProgress =
                    terminal.emulator.installer.BootstrapProgressCallback { progress ->
                        _bootstrapProgress.value = progress
                    }
                block(onProgress)
            } catch (exception: Exception) {
                LogUtil.e("ViewModel", "Bootstrap failed", exception)
                _bootstrapResult.value =
                    context.getString(R.string.bootstrap_error, exception.javaClass.simpleName)
            } finally {
                _bootstrapRunning.value = false
            }
        }
    }

    fun runBootstrap() {
        startBootstrapJob { onProgress ->
            val downloader =
                terminal.emulator.installer.BootstrapDownloader(
                    context,
                    onProgress = onProgress,
                )
            val (installer, secondStage) = bootstrapComponents(onProgress)
            val orchestrator =
                terminal.emulator.installer.BootstrapOrchestrator(
                    downloader,
                    installer,
                    secondStage,
                    onProgress = onProgress,
                )
            // 直接读取防抖后的值：DataStore 写入有 500ms 防抖，
            // 故用户在输入后立刻点安装时 first() 仍可能返回旧 URL。
            // 已编辑的输入框（即使被清空）优先于已存储的 URL。
            val url =
                if (bootstrapUrlEdited) {
                    bootstrapUrlEdits.replayCache.last()
                } else {
                    settingsRepository.bootstrapUrl.first()
                }
            val result = orchestrator.ensureBootstrap(url)
            _bootstrapResult.value = bootstrapOutcomeText(result)
        }
    }

    /**
     * 从 SAF URI 离线安装引导。用户经 `ActivityResultContracts.OpenDocument` 选择 .zip 文件；
     * 其内容被复制到缓存文件，再送入与在线路径相同的安装器管线
     * （installer.install → secondStage.run）。无需网络；已下载的引导 URL 被忽略。
     */
    fun installOffline(uri: android.net.Uri) {
        startBootstrapJob { onProgress ->
            val (installer, secondStage) = bootstrapComponents(onProgress)
            // 把 SAF URI 内容复制到临时缓存文件——安装器需要一个 File
            // （它会对 zip 做哈希与流式读取）。缓存目录总是可写，
            // 且会在系统压力下被清理。
            _bootstrapProgress.value = terminal.emulator.installer.BootstrapProgress.Downloading(0, 0)
            val cacheFile = java.io.File(context.cacheDir, "offline-bootstrap.zip")
            context.contentResolver.openInputStream(uri)?.use { input ->
                cacheFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            } ?: throw java.io.IOException("Failed to open bootstrap file")
            val result = installer.install(cacheFile)
            if (result.isSuccess) {
                val secondResult = secondStage.run()
                if (!secondResult.success) {
                    val failureDiagnostics = secondResult.errors.take(3).joinToString("\n") { "- $it" }
                    _bootstrapResult.value =
                        context.getString(
                            R.string.bootstrap_error,
                            failureDiagnostics.ifEmpty { "Postinst failed" },
                        )
                } else {
                    _bootstrapProgress.value = terminal.emulator.installer.BootstrapProgress.Complete
                    val headline = context.getString(R.string.bootstrap_installed_from_file)
                    _bootstrapResult.value = headline
                }
            } else {
                _bootstrapResult.value =
                    context.getString(
                        R.string.bootstrap_error,
                        result.exceptionOrNull()?.javaClass?.simpleName,
                    )
            }
            cacheFile.delete()
        }
    }

    fun setThemeName(name: String) = applyThemeSettings { settingsRepository.setThemeName(name) }

    fun setDayThemeName(name: String) = applyThemeSettings {
        settingsRepository.setDayThemeName(name)
    }

    fun setNightThemeName(name: String) = applyThemeSettings {
        settingsRepository.setNightThemeName(name)
    }

    fun setThemeMode(mode: String) = applyThemeSettings { settingsRepository.setThemeMode(mode) }

    fun setAppThemeMode(mode: String) = applyThemeSettings {
        settingsRepository.setAppThemeMode(mode)
    }

    /** 持久化一项主题设置，然后把整个主题重新应用到 bridge。由五个主题设置器共用。 */
    private fun applyThemeSettings(persist: suspend () -> Unit) {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            persist()
            runtime.applySettings()
        }
    }

    /**
     * 已编辑的引导 URL 文本。必须是 `MutableSharedFlow` 而非 `MutableStateFlow`：
     * 后者的初值就是首次 emission，会在 ViewModel 创建后经防抖把空串写进 DataStore，
     * 静默抹掉用户已保存的源。`replay = 1` 让安装按钮能读到最近一次编辑值。
     */
    private val bootstrapUrlEdits = MutableSharedFlow<String>(
        replay = 1,
        onBufferOverflow = BufferOverflow.DROP_OLDEST,
    )

    // 在 UI 线程写入、在 IO 协程读取；volatile 使可见性显式化，
    // 而不依赖隐式的 happens-before 关系。
    @Volatile private var bootstrapUrlEdited = false

    init {
        // 对自由文本设置防抖，使输入不会每次击键都写 DataStore
        // （每次写入都是完整的文件重写）。
        @OptIn(kotlinx.coroutines.FlowPreview::class)
        viewModelScope.launch {
            bootstrapUrlEdits.debounce(DEBOUNCE_MILLIS).distinctUntilChanged().collect { value ->
                settingsRepository.setBootstrapUrl(value)
            }
        }
    }

    /** Shell 启动入口经保存按钮直接写入（DESIGN :122 提供保存按钮），不检查文本。 */
    fun setShell(shell: String) {
        viewModelScope.launch { settingsRepository.setShell(shell) }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 四、键盘与硬件按键处理

    fun handleLayoutAwareHardwareKey(event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false

        // 只处理物理键盘。软键盘/输入法输入经 InputConnection（commitText）流动；
        // 在此拦截会把 CJK/语音组字变成原始拉丁字母。
        if ((event.flags and KeyEvent.FLAG_SOFT_KEYBOARD) != 0) return false
        if (event.deviceId == KeyCharacterMap.VIRTUAL_KEYBOARD) return false
        if (!event.isFromSource(InputDevice.SOURCE_KEYBOARD)) return false

        val keyCode = event.keyCode
        // 跳过纯修饰键的按压，交由视图处理。
        when (keyCode) {
            KeyEvent.KEYCODE_SHIFT_LEFT,
            KeyEvent.KEYCODE_SHIFT_RIGHT,
            KeyEvent.KEYCODE_CTRL_LEFT,
            KeyEvent.KEYCODE_CTRL_RIGHT,
            KeyEvent.KEYCODE_ALT_LEFT,
            KeyEvent.KEYCODE_ALT_RIGHT,
            KeyEvent.KEYCODE_META_LEFT,
            KeyEvent.KEYCODE_META_RIGHT,
            KeyEvent.KEYCODE_CAPS_LOCK,
            KeyEvent.KEYCODE_NUM_LOCK,
            KeyEvent.KEYCODE_SCROLL_LOCK,
            KeyEvent.KEYCODE_FUNCTION,
            -> return false
        }

        val meta = event.metaState
        val hasAltGr = (meta and KeyEvent.META_ALT_RIGHT_ON) != 0
        // Ctrl+键与左 Alt+键基于按键码（控制字节 / ESC 前缀），不依赖布局
        // ——交给编码器路径处理。AltGr 是例外：它产生的是组合字符。
        if ((meta and KeyEvent.META_CTRL_ON) != 0 && !hasAltGr) return false
        if ((meta and KeyEvent.META_ALT_ON) != 0 && !hasAltGr) return false

        val unicodeChar = event.getUnicodeChar(meta)
        if (unicodeChar <= 0) return false

        val bridge = runtime.bridge() ?: return false

        // 修饰键掩码只由工具栏粘滞状态构建。Shift 已由 getUnicodeChar 并入生成的字符。
        val state = _state.value
        val mask = KeyModifiers.fromStickyStates(state.ctrlState, state.altState)

        // 未上档码点是未施加任何修饰键的基准键：
        // 去掉 SHIFT 后重算字符，使编码器能侦测纯 Shift 变化
        // （如 Shift+; -> :）并避免多余的 Kitty shift。
        val unshiftedChar = event.getUnicodeChar(meta and KeyEvent.META_SHIFT_MASK.inv())
        val success = bridge.processKeyEvent(keyCode, mask.toByte(), 0, unicodeChar, unshiftedChar)
        if (success) {
            // 清除一次性（轻点）粘滞修饰键，使其不会延续到下一次击键。
            // 上方编码器已看到本次击键的激活修饰键；消费发生在编码之后。
            consumeOneShotModifiers()
            LogUtil.d(
                "TerminalViewModel",
                // 绝不记录字符本身——硬件键盘输入可能含密码，logcat 无差别记录。
                "handleLayoutAwareHardwareKey: keyCode=$keyCode mask=$mask",
            )
        }
        return success
    }

    fun toggleScrollMode() {
        _state.update { it.copy(scrollActive = !it.scrollActive) }
        // 把 scrollActive 同步到 SessionEntry，使渲染线程知道新输出时是否自动复位滚动。
        runtime.setScrollActive(_state.value.scrollActive)
    }

    fun createSession() {
        val surface = currentSurface
        if (surface == null || !surface.isValid) {
            LogUtil.e(
                "TerminalViewModel",
                "createSession: surface null or invalid, currentSurface=$currentSurface",
            )
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels <= 0 || surfaceHeightPixels <= 0) {
            LogUtil.e(
                "TerminalViewModel",
                "createSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels",
            )
            return
        }

        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            val currentSurfaceNow = currentSurface
            if (currentSurfaceNow == null || !currentSurfaceNow.isValid) {
                LogUtil.e(
                    "TerminalViewModel",
                    "createSession: surface became invalid before launch",
                )
                return@launch
            }
            try {
                val newId =
                    runtime.createSession(currentSurfaceNow, surfaceWidthPixels, surfaceHeightPixels)
                if (newId > 0) {
                    _state.update { current ->
                        // 运行期的 sessionIds 收集器可能已在此更新运行前
                        // 就并入了 newId（两者都跑在 IO 调度器上）
                        // ——distinct() 使列表不含重复 id
                        // （否则 LazyColumn 键冲突："Key N was already used"）。
                        val sortedIds = (current.sessions.map { it.id } + newId).distinct().sorted()
                        val displayIndex = sortedIds.indexOf(newId) + 1
                        val previousById = current.sessions.associateBy { it.id }
                        val sessions = sortedIds.map { id -> previousById[id] ?: SessionInfo(id = id) }
                        current.copy(
                            sessionId = newId,
                            isRunning = true,
                            title = context.getString(R.string.session_number, displayIndex),
                            selection = SelectionState(),
                            sessions = sessions,
                            activeSessionId = newId,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                } else {
                    LogUtil.e(
                        "TerminalViewModel",
                        "createSession: runtime returned invalid id=$newId",
                    )
                }
            } catch (exception: Exception) {
                LogUtil.e("TerminalViewModel", "createSession failed", exception)
            }
        }
    }

    fun switchSession(id: Long) {
        val surface = currentSurface
        if (surface == null || !surface.isValid) {
            LogUtil.e(
                "TerminalViewModel",
                "switchSession: surface null or invalid, currentSurface=$currentSurface",
            )
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels == 0 || surfaceHeightPixels == 0) {
            LogUtil.e(
                "TerminalViewModel",
                "switchSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels",
            )
            return
        }

        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            try {
                runtime.switchSession(id, surface, surfaceWidthPixels, surfaceHeightPixels)
            } catch (exception: Exception) {
                LogUtil.e("TerminalViewModel", "switchSession failed for id=$id", exception)
                return@launch
            }
            _state.update { current ->
                current.copy(
                    sessionId = id,
                    isRunning = true,
                    title =
                    runtime.state.value.title.ifEmpty {
                        val sortedIds = current.sessions.map { it.id }.sorted()
                        context.getString(R.string.session_number, sortedIds.indexOf(id) + 1)
                    },
                    activeSessionId = id,
                    selection = SelectionState(),
                    selectionAccent = runtime.accentColor,
                )
            }
            // 把 SCROLL 按钮锁重新同步到新的活动 SessionEntry：
            // 每个条目都以 scrollActive=false 起步，故不做此步，
            // 任何会话切换后该开关都会静默失去对新输出滚动复位的抑制
            // （状态说开，渲染线程说关）。
            runtime.setScrollActive(_state.value.scrollActive)
        }
    }

    fun closeSession() {
        closeSession(_state.value.activeSessionId)
    }

    fun closeSession(id: Long) {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            try {
                runtime.closeSession(id)
            } catch (exception: Exception) {
                // closeSession 绝不能逃逸到主线程的未捕获处理器：
                // BootGuard 会视其为崩溃并杀掉进程。
                // 原生侧能容忍未知/已死的会话。
                LogUtil.e("TerminalViewModel", "closeSession failed for id=$id", exception)
                return@launch
            }
            withContext(TerminalDispatchers.main) {
                _state.update { current ->
                    val remaining = current.sessions.filter { it.id != id }
                    if (remaining.isEmpty()) {
                        current.copy(
                            isRunning = false,
                            sessions = emptyList(),
                            activeSessionId = 0L,
                            selection = SelectionState(),
                        )
                    } else {
                        val renumbered = remaining.sortedBy { it.id }
                        val newActive =
                            if (current.activeSessionId == id) {
                                remaining.last().id
                            } else {
                                current.activeSessionId
                            }
                        val newActiveIndex = renumbered.indexOfFirst { it.id == newActive }
                        current.copy(
                            sessions = renumbered,
                            activeSessionId = newActive,
                            sessionId = newActive,
                            title =
                            runtime.state.value.title.ifEmpty {
                                context.getString(R.string.session_number, newActiveIndex + 1)
                            },
                            selection = SelectionState(),
                        )
                    }
                }
                refreshSessionMetas(force = true)
            }
        }
    }

    /** RIS 全重置当前会话：恢复终端初始状态并清空回滚，同时清除已选区（端点已失效）。 */
    fun resetActiveTerminal() {
        val id = _state.value.activeSessionId
        if (id == 0L) return
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            try {
                NativeBridge.resetTerminal(id)
            } catch (exception: Exception) {
                LogUtil.e("TerminalViewModel", "resetTerminal failed for id=$id", exception)
                return@launch
            }
            // VT 视口已归零：同步清零 Kotlin 侧滚动记账并通知 surface 重同步，
            // 否则残留偏移会在下次滚动时算出错误 delta。
            runtime.setScrollOffset(0)
            runtime.setScrollRemainderPx(0f)
            _state.update { it.copy(scrollEpoch = it.scrollEpoch + 1) }
            clearSelection()
            // 搜索高亮是渲染覆盖层，存于全局 RenderState，reset 后空网格上会残留旧框。
            // 经 Bridge 清除（仅拿 RENDER_STATE 锁，不进 registry，无锁序反转）。
            runtime.bridge()?.clearSearchHighlights()
            runtime.forceRender()
        }
    }

    fun setSessionTitle(title: String) {
        _state.update { it.copy(title = title) }
    }
}
