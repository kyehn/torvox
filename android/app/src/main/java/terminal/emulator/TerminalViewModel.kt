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
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.FontInfo
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.input.DeadKeyState
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.ModifierState
import terminal.emulator.input.toggled
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.runtime.pasteByteChunks
import terminal.emulator.runtime.truncatePasteText
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxConfiguration
import terminal.emulator.settings.TermuxPropertiesRepository
import terminal.emulator.ui.SelectionBounds
import terminal.emulator.ui.clampSelection
import terminal.emulator.ui.normalizeSelectionAnchors
import terminal.emulator.util.TerminalDispatchers
import terminal.emulator.util.formatWorkingDirectory
import terminal.emulator.util.runCatchingCancellable
import javax.inject.Inject

private const val CLIPBOARD_TEXT_MAX_LENGTH = 100_000

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

/** 会话抽屉条目：序号按列表位置渲染，[title] 为该会话终端标题（OSC 0/2）。 */
data class SessionInfo(
    val sessionId: Long,
    val title: String = "",
    /**
     * 该会话当前的工作目录（OSC 7 已解析为路径），未知时为空串。
     *
     * 与 [title] 分开存：很多 shell 并不设置标题，那时目录才是唯一能区分会话的信息。
     */
    val workingDirectory: String = "",
)

/** 非强制会话元数据刷新节流窗口；抽屉打开、关闭会话的强制刷新不受此限。 */
internal const val SESSION_META_REFRESH_THROTTLE_MS = 2000L

data class TerminalState(
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val selection: SelectionState = SelectionState(),
    val ctrlState: ModifierState = ModifierState.Off,
    val altState: ModifierState = ModifierState.Off,
    val scrollActive: Boolean = false,
    val sessions: List<SessionInfo> = emptyList(),
    val activeSessionId: Long = 0L,

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
    private val termuxProperties: TermuxPropertiesRepository,
    val runtime: TerminalRuntime,
) : ViewModel() {
    /**
     * `~/.termux/termux.properties` 的最新解析结果。
     *
     * 视图只读这里，不自己读文件：该文件是外部输入，读取时机（启动、回前台、
     * 设置变化）统一由运行期裁定，视图多处各自读会出现「键栏用新布局、边距还是旧的」。
     */
    val termuxConfiguration: StateFlow<TermuxConfiguration> = termuxProperties.configuration

    /**
     * 终端页是否在前台。
     *
     * `termux.properties` 的音量键 / Ctrl+Space / back-key 改写挂在 Activity 的分发上，
     * 设置页遮住终端时必须停用；否则在输入框里按音量键或 Ctrl+Space 也会写进终端。
     */
    @Volatile
    var terminalScreenActive: Boolean = false
        private set

    fun setTerminalScreenActive(active: Boolean) {
        terminalScreenActive = active
    }

    private val clipboardAccess = ClipboardAccess(context, tag = "TerminalViewModel")

    private val selectionManager = SelectionManager()

    // SCROLL 按钮锁最后同步到的会话 id：只在活动会话换人时才需要重新同步（见
    // runtime.state 收集器）。同会话内的普通状态更新不必重复推。
    private var lastScrollLockSyncedSessionId = 0L

    // 从运行期见到的最近网格尺寸；缩小时会钳位活跃选区（见 runtime.state 收集器）。
    @Volatile private var lastGridRows = 0

    @Volatile private var lastGridCols = 0

    // 视图侧回写的回滚长度缓存：选区行是绝对行（0 = 回滚顶部），
    // 钳位上界必须是绝对空间最后一行（回滚长度 + 视口行数 - 1），不得用视口行数。
    @Volatile private var cachedScrollbackLength = 0

    /**
     * 视图侧每次刷新回滚长度后回写，使选区钳位与视图用同一口径。
     *
     * 刻意不另起一个 `StateFlow` 镜像：组合侧没有订阅者（重组期间自己发起同步 JNI
     * 查询是最长可阻塞两次查询超时（约 1s）的调用），镜像只会在每次手势查询时多发一次通知。
     */
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

    /** 字体列表 UI 实际显示时枚举并发布列表，平时不触发。 */
    fun refreshFontList() = fontManager.refreshFontList()

    // ── 选区转发（实现在 SelectionManager） ──

    fun startSelection(row: Int, col: Int, pasteOnly: Boolean = false) =
        selectionManager.startSelection(row, col, pasteOnly)

    fun updateSelection(row: Int, col: Int) = selectionManager.updateSelection(row, col)

    fun endSelection() = selectionManager.endSelection()

    /** 快速拖动路径：计算选区边界而不写 Compose 状态；见 [SelectionManager.dragMove]。 */
    fun dragMove(draggingStart: Boolean, row: Int, col: Int, cachedMaxRow: Int, cachedMaxCol: Int): HandleDragResult? =
        selectionManager.dragMove(draggingStart, row, col, cachedMaxRow, cachedMaxCol)

    /** 把最后一次快速路径的拖动边界提交到 Compose 状态；见 [SelectionManager.commitDragBounds]。 */
    fun commitDragBounds() = selectionManager.commitDragBounds()

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
                state.copy(selection = current.copy(pasteOnly = false, dragging = true))
            }
        }
    }

    fun copySelectionToClipboard() = selectionManager.copySelectionToClipboard()

    /**
     * 菜单可用性判定用的选区文本：与 [copySelectionToClipboard]/[shareSelection]
     * 共用同一解析（[SelectionManager.resolvedSelectionText]），
     * 故「复制/分享项显示」与「动作取得到文本」不可能分叉。
     */
    fun selectionTextForAction(): String = selectionManager.resolvedSelectionText()

    fun clearSelection() = selectionManager.clearSelection()

    fun shareSelection() = selectionManager.shareSelection()

    fun selectAll() = selectionManager.selectAll()

    fun pasteFromClipboard() = selectionManager.pasteFromClipboard()

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

    fun writeToPty(sessionId: Long, data: ByteArray) {
        val isCommit =
            shouldResetScrollOnInput(data, _state.value.selection.let { it.active || it.dragging })
        // hasSelectionOrDrag 在 onUserInputForScrollSnap 内部也会检查；
        // 此处透传，使共用的辅助函数拥有最终守卫权。
        onUserInputForScrollSnap(isCommit)
        // 写失败的原因（会话不存在/正在关闭，或存活但无 bridge）已由 runtime 记全，
        // 此处不再重复一条泛化错误。
        runtime.writeToPty(sessionId, data)
    }

    /** 把字节直接送入 VT 解析器（供转义序列测试使用）。 */
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

    /** Ctrl 的当前粘滞态，供按住类输入（音量键虚拟 Ctrl）判断是否需要回退自己置的位。 */
    val ctrlModifierState: ModifierState
        get() = _state.value.ctrlState

    /**
     * 直接设定 Ctrl 粘滞态。
     *
     * 与 [cycleCtrlState]/[lockCtrlState] 分开是为让调用方表达「我要它处于这个状态」而不是
     * 「把它推到下一个状态」：音量键按下/抬起是一对事件，用 `cycle` 表达会在中途被用户的
     * 手动切换打乱——抬起时再 cycle 一次就把用户刚点亮的 Ctrl 关掉了。
     */
    fun setCtrlModifier(state: ModifierState) {
        if (_state.value.ctrlState == state) return
        _state.update { it.copy(ctrlState = state) }
    }

    fun consumeOneShotModifiers() {
        val currentState = _state.value
        var newCtrl = currentState.ctrlState
        var newAlt = currentState.altState
        if (newCtrl == ModifierState.Once) newCtrl = ModifierState.Off
        if (newAlt == ModifierState.Once) newAlt = ModifierState.Off
        // `Held`（音量键按住）由抬起事件回收，此处不消费。
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
        fun startSelection(row: Int, col: Int, pasteOnly: Boolean = false) {
            val anchor = SelectionAnchor(row, col)
            // 新一次长按必须丢弃上一轮快速拖动留下的实时边界，否则首个 MOVE
            // 会以旧选区为基准算出错误端点。
            lastDragBounds = null
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
        fun dragMove(
            draggingStart: Boolean,
            row: Int,
            col: Int,
            cachedMaxRow: Int,
            cachedMaxCol: Int,
        ): HandleDragResult? {
            val current = _state.value.selection
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
        internal fun syncDragBoundsToNativeThrottled(bounds: HandleDragResult) {
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
            _state.update { state ->
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
            _state.update { state ->
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
            val selection = _state.value.selection
            if (!selection.active) return ""
            // 重新提取而不信任缓存的 selectedText：若缓存文本为空
            // （某些回滚状态下原生 selection_text 为 null），
            // 按需提取保证菜单动作总是复制当前选区。
            return selection.selectedText.ifEmpty { extractSelectedText(selection) }
        }

        /** 选区边界的日志形式：空解析告警必须自带定位信息，否则等于没记。 */
        private fun selectionBoundsForLog(): String {
            val selection = _state.value.selection
            val start = selection.start
            val end = selection.end
            if (start == null || end == null) return "(无界)"
            return "(start=${start.row},${start.col} end=${end.row},${end.col} pasteOnly=${selection.pasteOnly})"
        }

        fun copySelectionToClipboard() {
            val rawText = resolvedSelectionText()
            if (rawText.isEmpty()) {
                // 不再静默：空解析是用户可见的「点了没反应」，必须留痕可查。
                LogUtil.warn("TerminalViewModel", "复制跳过: 选区文本为空 ${selectionBoundsForLog()}")
                return
            }
            val clipped = rawText.take(CLIPBOARD_TEXT_MAX_LENGTH)
            clipboardAccess.setClipboardText(clipped, label = "terminal selection")
            // 动作完成后关闭浮动菜单；保留高亮。
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }

        fun clearSelection() {
            // 同批作废未提交的拖动边界：输入法显隐、surface 销毁等路径会在拖动途中
            // 清选区，`finishHandleDrag` 不再到达。此后新建的选区若再提交一次拖动，
            // `commitDragBounds` 会把**上一段**的旧边界写进来，抽取与复制随即取到
            // 不相干的内容。
            lastDragBounds = null
            _state.update { it.copy(selection = SelectionState()) }
            syncSelectionToNative()
            // 强制立即重绘：否则原生侧的脏标志要等下一个 vsync 节拍，
            // 使陈旧的选区高亮在屏幕上残留长达一帧周期。
            runtime.forceRender()
        }

        private fun syncSelectionToNative() {
            val selection = _state.value.selection
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
                LogUtil.warn("TerminalViewModel", "分享跳过: 选区文本为空 ${selectionBoundsForLog()}")
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
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }

        fun selectAll() {
            // 上游 select_all：整个内容（回滚 + 视口）一次派生，native 侧已
            // 安装为终端选区，Kotlin 只消费回传的有序界限（不含尾部空行）。
            val bounds = runtime.bridge()?.selectAll()
            if (bounds == null || bounds.size != SELECTION_BOUNDS_LENGTH) {
                // 无内容/查询失败：按“无数据”处理，不伪造选择。显式记录：
                // 静默返回让调用方无法区分空缓冲与查询超时。
                LogUtil.warn("TerminalViewModel", "selectAll: no bounds (bridge/request failed), skipping")
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
            val bridge = runtime.bridge()
            if (bridge == null) {
                LogUtil.error("TerminalViewModel", "extractSelectedText: 无活动会话，选区文本未提取")
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
                LogUtil.error("TerminalViewModel", "extractSelectedText: 原生选区查询失败，选区文本未提取")
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
            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                // 空剪贴板与读取失败都不许无声：失败已由 ClipboardAccess 记日志，
                // 这里补的是「点粘贴却什么都不发生」的空剪贴板。
                val text =
                    clipboardAccess.clipboardText().getOrNull()
                        ?: run {
                            LogUtil.warn("TerminalViewModel", "粘贴跳过: 剪贴板无文本")
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
        fun executePaste(text: String) {
            // 目标会话在粘贴开始时定一次：分块入队是异步刷写，期间切会话
            // 会让粘贴尾部写进新会话。
            val sessionId = runtime.inputTargetSessionId
            val truncated = truncatePasteText(text)
            if (truncated.isEmpty()) {
                LogUtil.debug("TerminalViewModel", "粘贴跳过：空白或截断后无内容")
                _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
                return
            }
            val encoded =
                try {
                    NativeBridge.encodePaste(sessionId, truncated.toByteArray(Charsets.UTF_8))
                } catch (exception: Exception) {
                    LogUtil.warn("TerminalViewModel", "粘贴编码失败", exception)
                    null
                }
            if (encoded == null || encoded.isEmpty()) {
                LogUtil.warn("TerminalViewModel", "粘贴跳过：编码无输出")
                _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
                return
            }
            // `pasteSink` 由 Surface 注入批缓冲（内部 `synchronized`），任意线程可调。
            for (chunk in pasteByteChunks(encoded)) {
                pasteSink(sessionId, chunk)
            }
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }
    }

    /** 粘贴字节出口：默认同步写，Surface 侧注入批缓冲后走异步合并写。会话 id 随块传入，不在刷写时解析。 */
    var pasteSink: (Long, ByteArray) -> Unit = { sessionId, data -> runtime.writeToPty(sessionId, data) }

    // ══════════════════════════════════════════════════════════════════════
    // 二之二、字体管理
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有字体加载、字号/家族设置与字体文件安装。内部类：
     * 经外层 ViewModel 访问字体 StateFlow、runtime、settingsRepository 与 context。
     * 「应用已存字体」与「列举字体列表」是两条路径：前者随会话启动，
     * 后者只在字体列表 UI 实际显示时（`refreshFontList` 由设置页 `LaunchedEffect` 触发）。
     */
    inner class FontManager {
        /**
         * DESIGN 字体选择节：native 应用返回 false 即设置数据错误，
         * 输出日志并清除该设置使会话回落默认。null 表示无会话可验证，不处理。
         */
        private suspend fun clearUnknownFontFamily(requestedFamily: String, applied: Boolean?) {
            if (applied != false) return
            LogUtil.error("Font", "Unknown font family, clearing setting: $requestedFamily")
            settingsRepository.clearFontFamily()
        }

        /**
         * 在途加载的协程句柄。字体列表 UI 可在短时间内多次进入，
         * 若无此门闩每次进入都会再启一个枚举并发跑
         * setExtraFontPaths/listFontFamilies/_fontInfo 写入。
         */
        private var fontLoadJob: Job? = null

        /**
         * 会话启动路径：把已存字族应用到本会话的 bridge（bridge 按会话持有，
         * 每个新会话都要应用一次）。只做应用，不列举——列表是 UI 的事。
         */
        fun applyStoredFontSettings() {
            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                val bridge = runtime.bridge()
                if (bridge == null) {
                    // 会话已登记但 bridge 尚未就绪（spawn 前的瞬间）：跳过本轮，
                    // 后续状态更新会再次触发本函数。
                    return@launch
                }
                // 先注册用户投放目录再应用，使其字体在 bridge 上可见
                // （重复注册为空操作）；主/次路径都纳入。
                val fontDirs = terminal.emulator.termuxFontDirs(context).filter { it.isDirectory }.map { it.absolutePath }
                if (fontDirs.isNotEmpty()) bridge.setExtraFontPaths(fontDirs)
                val storedFamily = settingsRepository.fontFamily.first()
                clearUnknownFontFamily(
                    storedFamily,
                    bridge.setFontFamily(
                        terminal.emulator.resolveEffectiveFontFamily(storedFamily),
                    ),
                )
            }
        }

        /**
         * 字体列表 UI 路径：枚举并发布可用列表、默认字体名与字体信息。
         * 只在字体列表实际显示时调用（设置页打开字体选择对话框那一刻），
         * 不由会话状态发射触发。
         *
         * 进程内只跑一轮（DESIGN：字体列表被缓存直到应用关闭）：`fonts.xml` 与用户
         * 投放目录在进程存活期间不会变，重跑只会反复走一遍 JNI 列举与目录注册。
         */
        fun refreshFontList() {
            // 并发闩只管「别并发起多轮」；缓存闩只在**真的枚举完成**后置位——
            // 提前置位会让「会话尚未就绪」这一次早退把整进程都锁成空列表，
            // 之后再点开选择框也不再重试。
            if (fontListLoaded || fontLoadJob?.isActive == true) return
            fontLoadJob = viewModelScope.launch(TerminalDispatchers.inputOutput) {
                try {
                    val bridge = runtime.bridge()
                    if (bridge == null) {
                        // 会话已登记但 bridge 尚未就绪（spawn 前的瞬间）：跳过本轮。
                        // _availableFonts 仍为空，直接交给 availableFontFamilies
                        // 会因空列表抛致命异常，把「还没准备好」变成「进程崩溃」。
                        return@launch
                    }
                    // 先注册用户投放目录再列举，使其字体家族即使在早于
                    // Runtime.start() 完成时也被纳入（重复注册为空操作）；主/次路径都纳入。
                    val fontDirs = terminal.emulator.termuxFontDirs(context).filter { it.isDirectory }.map { it.absolutePath }
                    if (fontDirs.isNotEmpty()) bridge.setExtraFontPaths(fontDirs)
                    val allFonts =
                        terminal.emulator.settings.availableFontFamilies(
                            bridge.listFontFamilies().orEmpty(),
                        )
                    _availableFonts.value = allFonts
                    fontListLoaded = true
                    // 空即「原生未上报」，不得改用列表首项冒充：DESIGN 要求主字体为空时
                    // 取 fonts.xml 的 monospace 家族，列表首项是字体库枚举顺序，与之无关。
                    // 显示侧本就对空值有回落（FontFamilySelectors 取列表首项仅作标签）。
                    publishFontState(bridge)
                } catch (fatal: IllegalStateException) {
                    if (fatal is kotlinx.coroutines.CancellationException) throw fatal
                    // 系统 fonts.xml 缺失或不可解析：按 DESIGN 记录日志并崩溃退出，
                    // 不得静默回退为空列表。
                    LogUtil.error("TerminalViewModel", "Fatal: system fonts.xml unreadable", fatal)
                    throw fatal
                } catch (exception: Exception) {
                    if (exception is kotlinx.coroutines.CancellationException) throw exception
                    LogUtil.error("TerminalViewModel", "Failed to load font list", exception)
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
                    publishFontState(bridge)
                }
            }
        }

        fun setFontFamily(family: String) {
            viewModelScope.launch(TerminalDispatchers.inputOutput) {
                try {
                    LogUtil.debug("Font", "Setting font family: $family")
                    settingsRepository.setFontFamily(family)
                    val applied = runtime.applyFontSettings()
                    val bridge = runtime.bridge()
                    clearUnknownFontFamily(family, applied)
                    publishFontState(bridge)
                    LogUtil.debug("Font", "Font applied: ${_defaultFontName.value}")
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
                    if (exception is kotlinx.coroutines.CancellationException) throw exception
                    LogUtil.error("Font", "setFontFamily failed for $family", exception)
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
        // 手柄拖动期间两次原生推送之间的最小间隔：一帧（60Hz），即高亮最多落后
        // 手指一帧。更长（原先 50ms≈3 帧）在快速拖动时可被看出「不跟手」，
        // 更短不会更快——notifyRender 会立刻让渲染线程做一次整屏重绘，
        // 这个间隔正是拖动期间的渲染节奏上限。
        private const val DRAG_NATIVE_SYNC_INTERVAL_MS = 16L
        private const val TIMEOUT_MILLIS = 5_000L
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
                    runtime.screenWidthDp,
                ),
            ),
        )

    private val _availableFonts = MutableStateFlow<List<String>>(emptyList())
    val availableFonts: StateFlow<List<String>> = _availableFonts.asStateFlow()

    private val _defaultFontName = MutableStateFlow("")
    val defaultFontName: StateFlow<String> = _defaultFontName.asStateFlow()

    private val _fontInfo = MutableStateFlow<FontInfo?>(null)
    val fontInfo: StateFlow<FontInfo?> = _fontInfo.asStateFlow()

    /**
     * 从原生发布字体族名与字体信息，两个字段总是同批写入。
     *
     * null 即「原生尚未上报」：设置页据此只显示族名与字号，不伪造单元格尺寸。
     */
    private fun publishFontState(bridge: Bridge?) {
        _defaultFontName.value = bridge?.getDefaultFontName().orEmpty()
        _fontInfo.value = bridge?.getFontInfo()
    }

    init {
        // 首次启动：固定一个按设备自适应的默认字号，使网格在用户触碰字号滑块前即可读。
        viewModelScope.launch {
            // 屏幕宽与字号上界同源（runtime.screenWidthDp → resources.configuration）：
            // 自建 widthPixels/density 换算是窗口物理宽，与上界用的 configuration
            // 在系统字体缩放/多窗口下不同值，自适应默认字号会按另一个宽度推导。
            settingsRepository.applyFirstLaunchDefaultFontSize(runtime.screenWidthDp)
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
                val previousById = _state.value.sessions.associateBy { it.sessionId }
                val sessions = sortedIds.map { sessionId ->
                    previousById[sessionId]
                        ?: SessionInfo(sessionId = sessionId)
                }
                val active = runtimeState.activeSessionId
                // SCROLL 按钮锁是 Compose 状态，每个 SessionEntry 各存一份镜像。
                // 活动会话一换就把它对齐：新建会话的条目以 scrollActive=false 起步，
                // 而前一个会话关闭后接替上来的条目带着**它自己**的锁——任一方向不
                // 同步，按钮都会显示锁定而渲染线程并不抑制新输出引起的滚动复位。
                // 放在这里而不是各切换入口：三处入口（switchSession / createSession /
                // 接替）共用这一条，少一份就漏一处。
                if (active != 0L && active != lastScrollLockSyncedSessionId) {
                    lastScrollLockSyncedSessionId = active
                    runtime.setScrollActive(_state.value.scrollActive)
                }
                if (active != 0L) {
                    val displayIndex = sortedIds.indexOf(active) + 1
                    val title =
                        runtimeState.title.ifEmpty { context.getString(R.string.session_number, displayIndex) }
                    // 用 _state.update（CAS）而非读-改-写：
                    // IO 调度器上的 createSession/switchSession 也会更新 _state，
                    // 此处非原子的写入会覆盖它们刚提交的会话列表。
                    _state.update { current ->
                        current.copy(
                            isRunning = runtimeState.isRunning,
                            title = title,
                            sessions = sessions,
                            activeSessionId = active,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                    if (runtime.state.value.sessionIds.isNotEmpty()) {
                        // 会话启动只应用已存字体（bridge 按会话持有）；列表枚举是
                        // 字体列表 UI 的事，不在此触发。
                        fontManager.applyStoredFontSettings()
                        // 字体查询是同步 JNI，不得在 Main.immediate 收集器上执行。
                        viewModelScope.launch(TerminalDispatchers.inputOutput) {
                            publishFontState(runtime.bridge())
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
     * 全部在 IO 调度器上完成，成功提示由本方法自己切回主线程弹出（与 `setFontFamily`
     * 同款）：把回调交给界面层就得在契约里写明「非主线程」，而调用方一旦在回调里
     * 直接 `Toast.show()` 就会因无 Looper 抛 `IllegalStateException` 杀掉进程。
     * 有条目删不掉时记错误日志且**不**提示成功——抛异常会经协程默认处理器杀掉整个
     * 进程，而无条件提示又会让用户被告知「已清除」而 `settings.preferences_pb` 原封不动。
     */
    fun clearAppData() {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            // 先停掉防抖写入：用户在 300ms 窗口内改完 URL 就点清除，
            // 落盘会发生在删除之后并重建 preferences_pb，清除静默不生效。
            settingsRepository.dropPendingBootstrapUrlEdits()
            try {
                // 进程级 DataStore 单例仍在运行：删后必须重建，
                // 否则下一次设置写入失败。
                val watchedDirs = listOf("prefs")
                val undeletedWatchedDirs =
                    watchedDirs.filterNot { deletedOrAbsent(context.getDir(it, Context.MODE_PRIVATE)) }
                // listFiles() 返回 null 是 I/O 失败而非「目录为空」：按未清除记账。
                val cacheEntries = context.cacheDir.listFiles()
                val undeletedCacheEntries =
                    cacheEntries?.filterNot { deletedOrAbsent(it) }?.map { it.name }
                        ?: listOf("${context.cacheDir.name}(不可枚举)")
                val survivors = undeletedWatchedDirs + undeletedCacheEntries
                // 重建必须无条件执行：删除是逐目录独立的，`prefs` 删成功而 `boot_state`
                // 残留时若就此返回，进程级 DataStore 单例将因父目录消失而写不进去，
                // 此后所有设置写入全失败——比不清除更糟。
                watchedDirs.forEach { context.getDir(it, Context.MODE_PRIVATE) }
                if (survivors.isNotEmpty()) {
                    LogUtil.error("TerminalViewModel", "clear app data incomplete, undeleted or unreadable: $survivors")
                    return@launch
                }
                kotlinx.coroutines.withContext(TerminalDispatchers.main) {
                    android.widget.Toast.makeText(
                        context,
                        context.getString(R.string.clear_app_data_done),
                        android.widget.Toast.LENGTH_LONG,
                    )
                        .show()
                }
            } finally {
                // 无论成败都重新接受编辑：否则清除后用户再改 URL 永不落盘。
                settingsRepository.rearmBootstrapUrlEdits()
            }
        }
    }

    /** 目录已删除或本就不存在返回 true；存在却删不掉返回 false。 */
    private fun deletedOrAbsent(file: java.io.File): Boolean = !file.exists() || file.deleteRecursively()

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
        val sessionIds = _state.value.sessions.map { it.sessionId }.sorted()
        if (sessionIds.isEmpty()) return
        // 消费节流窗口须在空表早退之后：空列表本无事可做，占掉窗口会让紧跟着的
        // 首个真实会话错过一次标题回填。
        lastMetaRefreshMs = SystemClock.uptimeMillis()
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            // 一次批量查询取代逐会话往返：N 个会话原本是 N 次同步 JNI。
            val titles =
                runCatchingCancellable { NativeBridge.getSessionTitles(sessionIds.toLongArray()) }
                    .getOrNull()
            // 目录与标题同批取回：两者都由一次会话列表刷新消费，分两次往返只是
            // 把同一次 JNI 成本付两遍。
            val directories =
                runCatchingCancellable { NativeBridge.getSessionPwds(sessionIds.toLongArray()) }
                    .getOrNull()
            val fresh = sessionIds.mapIndexed { index, sessionId ->
                SessionInfo(
                    sessionId = sessionId,
                    title = titles?.getOrNull(index).orEmpty(),
                    workingDirectory =
                    formatWorkingDirectory(directories?.getOrNull(index).orEmpty()),
                )
            }
            withContext(TerminalDispatchers.main) {
                _state.update { current ->
                    if (current.sessions.map { it.sessionId }.sorted() != sessionIds) {
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
        LogUtil.debug("TerminalViewModel", "ensureDefaultSession: creating default session")
        createSession()
    }

    fun setBootstrapUrl(url: String) {
        bootstrapUrlEdited = true
        settingsRepository.recordBootstrapUrlEdit(url)
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

                // 编排器已把每条失败收敛成只含异常类名的文案（原始消息可能嵌入引导 URL，
                // 故不外泄），这里再取一次类名会把那层收敛整个抹掉：真机上 `file://`
                // 被 SELinux 拒绝时用户只看到「安装失败：Exception」，无从判断该换
                // 预设、换 https 还是改从文件安装。裸异常才退回类名。
                else -> context.getString(
                    R.string.bootstrap_error,
                    exception.message ?: exception.javaClass.simpleName,
                )
            }
        },
    )

    /** 在线与离线安装路径共用的安装器与二阶段执行器配对；目录取自三处入口共用的 [terminal.emulator.installer.bootstrapDirectories]。 */
    private fun bootstrapComponents(
        onProgress: terminal.emulator.installer.BootstrapProgressCallback,
    ): Pair<
        terminal.emulator.installer.BootstrapInstaller,
        terminal.emulator.installer.SecondStageRunner,
        > {
        val directories = terminal.emulator.installer.bootstrapDirectories(context)
        return terminal.emulator.installer.BootstrapInstaller(
            prefixDir = directories.prefix,
            homeDir = directories.home,
            stagingDir = directories.staging,
            onProgress = onProgress,
        ) to terminal.emulator.installer.SecondStageRunner(
            prefixDir = directories.prefix,
            homeDir = directories.home,
            onProgress = onProgress,
        )
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
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.error("TerminalViewModel", "Bootstrap failed", exception)
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
            // 直接读取防抖后的值：DataStore 写入有防抖
            // （terminal.emulator.settings.SettingsRepository.DEBOUNCE_MILLIS），
            // 故用户在输入后立刻点安装时 first() 仍可能返回旧 URL。此前此处硬写
            // 「500ms」而实际是 300ms，改动后就会漂移，故改为引用常量。
            // 已编辑的输入框（即使被清空）优先于已存储的 URL。
            val edited = settingsRepository.latestBootstrapUrlEdit()
            val url =
                if (bootstrapUrlEdited && edited != null) {
                    edited
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
            try {
                context.contentResolver.openInputStream(uri)?.use { input ->
                    cacheFile.outputStream().use { output ->
                        terminal.emulator.installer.copyBootstrapArchive(
                            input = input,
                            output = output,
                            onCopied = { copied ->
                                _bootstrapProgress.value =
                                    terminal.emulator.installer.BootstrapProgress.Downloading(copied, 0)
                            },
                        )
                    }
                } ?: throw java.io.IOException("Failed to open bootstrap file")
            } catch (copyError: Exception) {
                // 部分写入的归档不是合法 zip；留着只会占满 cacheDir。
                cacheFile.delete()
                throw copyError
            }
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

    /**
     * 持久化一项主题设置，然后把整个主题重新应用到 bridge。由五个主题设置器共用。
     *
     * 存储里失效的主题标识由运行期一处清完（`clearUnknownThemeNames`，三个键一并
     * 查），此处 MUST NOT 再兜一层异常重试：那一层捕获不到真正会抛的那个异常类型，
     * 反而会在原生字节布局出问题时把用户的主题重写掉——把契约破裂藏成「设置已修复」。
     */
    private fun applyThemeSettings(persist: suspend () -> Unit) {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            persist()
            runtime.applySettings()
        }
    }

    /** 字体列表是否已枚举过（进程内一次，见 [refreshFontList]）。 */
    @Volatile private var fontListLoaded = false

    // 在 UI 线程写入、在 IO 协程读取；volatile 使可见性显式化，
    // 而不依赖隐式的 happens-before 关系。
    @Volatile private var bootstrapUrlEdited = false

    /** 死键（组合符）状态：组合键本身不产生输出，等下一次击键合成。 */
    private val deadKeys = terminal.emulator.input.DeadKeyState()

    /** Shell 启动入口经保存按钮直接写入（DESIGN.md 设置节 Shell 启动入口项），不检查文本。 */
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

        val metaState = event.metaState
        val hasAltGr = (metaState and KeyEvent.META_ALT_RIGHT_ON) != 0
        // Ctrl+键与左 Alt+键基于按键码（控制字节 / ESC 前缀），不依赖布局
        // ——交给编码器路径处理。AltGr 是例外：它产生的是组合字符。
        if ((metaState and KeyEvent.META_CTRL_ON) != 0 && !hasAltGr) return false
        if ((metaState and KeyEvent.META_ALT_ON) != 0 && !hasAltGr) return false

        val unicodeChar = event.getUnicodeChar(metaState)
        if (unicodeChar == 0) return false

        // 死键（组合符）本身不产生字符：先记下，等下一次击键合成。丢掉它会让
        // AZERTY / US-International / Nordic 等布局只能打出裸字母。
        if (deadKeys.isDead(unicodeChar)) {
            deadKeys.hold(unicodeChar)
            return true
        }

        // AltGr 的组合字符已由键位表合成完毕，与待合成的组合键无关。
        val accent = if (hasAltGr) 0 else deadKeys.take()
        val composed = deadKeys.composeWith(accent, unicodeChar)
        if (accent != 0 && composed == 0) {
            LogUtil.warn("TerminalViewModel", "dead key $accent has no combination with $unicodeChar; accent dropped")
        }
        val outChar = if (composed != 0) composed else unicodeChar
        if (outChar <= 0) return false

        val bridge = runtime.bridge() ?: return false

        // 修饰键掩码只由工具栏粘滞状态构建。Shift 已由 getUnicodeChar 并入生成的字符。
        val state = _state.value
        val mask = KeyModifiers.fromStickyStates(state.ctrlState, state.altState)

        val success =
            bridge.processKeyEvent(
                keyCode,
                mask,
                0,
                outChar,
                // 无修饰码点：Shift 已并入 outChar，把它单独给出供上游编码器
                // 识别该键的档位。
                event.getUnicodeChar(0),
            )
        if (success) {
            // 硬件输入同样经过共用的贴底点，否则粘滞 SCROLL 永不被物理键盘解除。
            onUserInputForScrollSnap(
                isCommit = keyCode in terminal.emulator.ui.TerminalInputEncoder.enterKeyCodes,
            )
            // 清除一次性（轻点）粘滞修饰键，使其不会延续到下一次击键。
            // 上方编码器已看到本次击键的激活修饰键；消费发生在编码之后。
            consumeOneShotModifiers()
            LogUtil.debug(
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
            LogUtil.error(
                "TerminalViewModel",
                "createSession: surface null or invalid, currentSurface=$currentSurface",
            )
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels <= 0 || surfaceHeightPixels <= 0) {
            LogUtil.error(
                "TerminalViewModel",
                "createSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels",
            )
            return
        }

        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            val currentSurfaceNow = currentSurface
            if (currentSurfaceNow == null || !currentSurfaceNow.isValid) {
                LogUtil.error(
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
                        val sortedIds = (current.sessions.map { it.sessionId } + newId).distinct().sorted()
                        val displayIndex = sortedIds.indexOf(newId) + 1
                        val previousById = current.sessions.associateBy { it.sessionId }
                        val sessions = sortedIds.map { sessionId ->
                            previousById[sessionId]
                                ?: SessionInfo(sessionId = sessionId)
                        }
                        current.copy(
                            isRunning = true,
                            title = context.getString(R.string.session_number, displayIndex),
                            selection = SelectionState(),
                            sessions = sessions,
                            activeSessionId = newId,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                } else {
                    LogUtil.error(
                        "TerminalViewModel",
                        "createSession: runtime returned invalid id=$newId",
                    )
                }
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.error("TerminalViewModel", "createSession failed", exception)
            }
        }
    }

    fun switchSession(sessionId: Long) {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            // 协程内重读（同 createSession）：锁外捕获的 surface 在派发到 IO 调度器
            // 期间可能已失效或被换掉。
            val surface = currentSurface
            if (surface == null || !surface.isValid) {
                LogUtil.error(
                    "TerminalViewModel",
                    "switchSession: surface null or invalid, currentSurface=$currentSurface",
                )
                return@launch
            }
            val surfaceWidthPixels = surfaceWidth
            val surfaceHeightPixels = surfaceHeight
            if (surfaceWidthPixels == 0 || surfaceHeightPixels == 0) {
                LogUtil.error(
                    "TerminalViewModel",
                    "switchSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels",
                )
                return@launch
            }
            val switched =
                try {
                    runtime.switchSession(sessionId, surface, surfaceWidthPixels, surfaceHeightPixels)
                } catch (exception: Exception) {
                    if (exception is kotlinx.coroutines.CancellationException) throw exception
                    LogUtil.error("TerminalViewModel", "switchSession failed for id=$sessionId", exception)
                    return@launch
                }
            // 运行期中止了切换（会话已关、surface 失效、渲染线程起不来）：
            // 此时活动会话仍是别的 id，发布 activeSessionId=id 会让 UI 状态与
            // runtime.state/inputTargetSessionId 分裂，敲键将送往未显示的会话。
            if (!switched) {
                LogUtil.warn("TerminalViewModel", "switchSession aborted by runtime for id=$sessionId")
                return@launch
            }
            _state.update { current ->
                current.copy(
                    isRunning = true,
                    title =
                    runtime.state.value.title.ifEmpty {
                        val sortedIds = current.sessions.map { it.sessionId }.sorted()
                        context.getString(R.string.session_number, sortedIds.indexOf(sessionId) + 1)
                    },
                    activeSessionId = sessionId,
                    selection = SelectionState(),
                    selectionAccent = runtime.accentColor,
                )
            }
        }
    }

    /**
     * 切到相邻会话（`shortcut.next-session` / `shortcut.previous-session`）。
     *
     * 顺序取会话表的既有顺序（按 id 升序，与抽屉里显示的顺序一致），循环：
     * 最后一个的下一个回到第一个。只有一个会话时什么都不做——「切换到自身」
     * 会白走一次 surface 重挂载，屏幕闪一下。
     */
    fun switchToAdjacentSession(forward: Boolean) {
        val sessions = _state.value.sessions
        if (sessions.size < 2) return
        val activeIndex = sessions.indexOfFirst { it.sessionId == _state.value.activeSessionId }
        if (activeIndex < 0) return
        val step = if (forward) 1 else -1
        val target = sessions[Math.floorMod(activeIndex + step, sessions.size)].sessionId
        switchSession(target)
    }

    /**
     * 发送一个不带修饰键的按键。
     *
     * 供「返回键映射为 ESC」「音量键映射为方向键」这类**不经过输入法**的通道使用：
     * 它们不是用户在键盘上敲的字符，而是宿主把平台按键改写成终端按键。
     *
     * 返回 false 表示没有会话或原生拒绝了该键（编码器不产生字节），调用方应把事件
     * 放回平台默认路径——否则音量键会被静默吞掉，用户既调不了音量也发不出方向键。
     */
    fun sendKey(keyCode: Int, modifiers: Int = 0): Boolean {
        val bridge = runtime.bridge() ?: return false
        return bridge.processKeyEvent(
            keyCode = keyCode,
            modifiers = modifiers,
            action = KeyEvent.ACTION_DOWN,
            unicodeChar = 0,
            unshiftedChar = 0,
        )
    }

    fun closeSession() {
        closeSession(_state.value.activeSessionId)
    }

    fun closeSession(sessionId: Long) {
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            // 关闭资格必须在 closeSession **之前**捕获：runtime 在关掉活动会话时会切到
            // 接替会话并推 runtime.state，而该 state 的收集器跑在主线程、其 runnable
            // 先于下面的 withContext(main) 入队，会先把 _state.activeSessionId 改成接替
            // id。若在块内再读，closedActive 恒为 false——清选区分支永不执行，屏幕上
            // 留下无手柄、无菜单的幽灵反色（复制还会取到不相干区域）。
            val closedActive = _state.value.activeSessionId == sessionId
            try {
                runtime.closeSession(sessionId)
            } catch (exception: Exception) {
                // closeSession 绝不能逃逸到主线程的未捕获处理器：
                // 崩溃会让整个会话进程不可用。
                // 原生侧能容忍未知/已死的会话。
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.error("TerminalViewModel", "closeSession failed for id=$sessionId", exception)
                return@launch
            }
            withContext(TerminalDispatchers.main) {
                _state.update { current ->
                    val remaining = current.sessions.filter { it.sessionId != sessionId }
                    if (remaining.isEmpty()) {
                        current.copy(
                            isRunning = false,
                            sessions = emptyList(),
                            activeSessionId = 0L,
                            selection = SelectionState(),
                        )
                    } else {
                        val renumbered = remaining.sortedBy { it.sessionId }
                        val newActive =
                            if (current.activeSessionId == sessionId) {
                                remaining.last().sessionId
                            } else {
                                current.activeSessionId
                            }
                        val newActiveIndex = renumbered.indexOfFirst { it.sessionId == newActive }
                        current.copy(
                            sessions = renumbered,
                            activeSessionId = newActive,
                            title =
                            runtime.state.value.title.ifEmpty {
                                context.getString(R.string.session_number, newActiveIndex + 1)
                            },
                            // 只在关掉的就是活动会话时才清选区：此前无条件清空，
                            // 于是「关掉一个后台会话」会顺手抹掉**活动**会话的选区状态，
                            // 而原生侧的 setSelection 仍在——屏幕上留下无手柄、无菜单
                            // 的幽灵反色，复制还会取到不相干的区域。清选区必须与
                            // 原生同步，否则同一条路径也会留下幽灵反色。
                            selection = if (closedActive) SelectionState() else current.selection,
                        )
                    }
                }
                if (closedActive) {
                    selectionManager.clearSelection()
                }
                refreshSessionMetas(force = true)
            }
        }
    }

    /** RIS 全重置当前会话：恢复终端初始状态并清空回滚，同时清除已选区（端点已失效）。 */
    fun resetActiveTerminal() {
        val sessionId = _state.value.activeSessionId
        if (sessionId == 0L) return
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            try {
                NativeBridge.resetTerminal(sessionId)
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.error("TerminalViewModel", "resetTerminal failed for id=$sessionId", exception)
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
}
