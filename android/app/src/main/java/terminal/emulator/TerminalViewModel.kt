package terminal.emulator

import android.content.Context
import android.net.Uri
import android.os.SystemClock
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.Surface
import android.widget.Toast
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.FontInfo
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.input.DeadKeyState
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.ModifierState
import terminal.emulator.input.toggled
import terminal.emulator.installer.BootstrapProgress
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.MacroParser
import terminal.emulator.settings.MacroStep
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxConfiguration
import terminal.emulator.settings.TermuxPropertiesRepository
import terminal.emulator.util.TerminalDispatchers
import terminal.emulator.util.formatWorkingDirectory
import terminal.emulator.util.runCatchingCancellable
import java.io.File
import javax.inject.Inject

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

    /**
     * 宏键的文本来源：虚拟键盘的字符表。
     *
     * 与实体键盘同源（平台自己的布局表），故「Shift+f → F」「AltGr → 组合字符」
     * 这些细节不必在应用侧另写一份。
     */
    private val macroCharacterMap by lazy { KeyCharacterMap.load(KeyCharacterMap.VIRTUAL_KEYBOARD) }

    private val _state = MutableStateFlow(TerminalState())
    val state: StateFlow<TerminalState> = _state.asStateFlow()

    private val selectionManager =
        SelectionManager(
            terminalState = _state,
            runtime = runtime,
            context = context,
            scope = viewModelScope,
        )

    /** 视图侧每次刷新回滚长度后回写（实现在 SelectionManager）。 */
    internal fun updateScrollbackLength(length: Int) = selectionManager.updateScrollbackLength(length)

    // SCROLL 按钮锁最后同步到的会话 id：只在活动会话换人时才需要重新同步（见
    // runtime.state 收集器）。同会话内的普通状态更新不必重复推。
    private var lastScrollLockSyncedSessionId = 0L

    // 从运行期见到的最近网格尺寸；缩小时会钳位活跃选区（见 runtime.state 收集器）。
    @Volatile private var lastGridRows = 0

    @Volatile private var lastGridCols = 0

    // 视图侧回写的回滚长度缓存：选区行是绝对行（0 = 回滚顶部），
    // 钳位上界必须是绝对空间最后一行（回滚长度 + 视口行数 - 1），不得用视口行数。
    private val fontManager =
        FontManager(
            runtime = runtime,
            settingsRepository = settingsRepository,
            context = context,
            scope = viewModelScope,
        )

    private val themeManager =
        ThemeManager(
            settingsRepository = settingsRepository,
            runtime = runtime,
            scope = viewModelScope,
        )

    // ── 引导转发（实现在 BootstrapManager） ──

    private val bootstrapManager =
        BootstrapManager(
            settingsRepository = settingsRepository,
            context = context,
            scope = viewModelScope,
        )

    /** 引导安装是否在进行中。 */
    val bootstrapRunning: StateFlow<Boolean> = bootstrapManager.bootstrapRunning

    /** 最近一次引导安装的结果文案。 */
    val bootstrapResult: StateFlow<String?> = bootstrapManager.bootstrapResult

    /** 最近一次引导安装的进度。 */
    val bootstrapProgress: StateFlow<BootstrapProgress?> = bootstrapManager.bootstrapProgress

    fun setBootstrapUrl(url: String) = bootstrapManager.setBootstrapUrl(url)

    fun runBootstrap() = bootstrapManager.runBootstrap()

    fun installOffline(uri: Uri) = bootstrapManager.installOffline(uri)

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
    fun beginHandleDragOnPasteOnly() = selectionManager.beginHandleDragOnPasteOnly()

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

    /** 粘贴字节出口（实现在 SelectionManager）。 */
    var pasteSink: (Long, ByteArray) -> Unit
        get() = selectionManager.pasteSink
        set(value) {
            selectionManager.pasteSink = value
        }

    companion object {
        private const val TIMEOUT_MILLIS = 5_000L
    }

    /**
     * 当前可用的 Surface。
     *
     * 赋值即「Surface 已交付」的唯一时刻，故 setter 在此补建挂起的 Failsafe 会话
     * （见 [createPendingFailsafeSession]）：快捷方式以标准 launchMode 新建 Activity
     * 实例时，新实例的 Surface 要等布局之后才交付；应用从后台被拉起时同样如此。
     * 只在非空时补建——销毁路径把 Surface 置空不代表「有新的可用窗口」。
     */
    @Volatile
    var currentSurface: Surface? = null
        set(value) {
            field = value
            if (value != null) {
                createPendingFailsafeSession()
            }
        }

    @Volatile var surfaceWidth: Int = 0

    @Volatile var surfaceHeight: Int = 0

    /**
     * 待建的 Failsafe 会话：外部 Intent 到达时应用已在运行，但建会话所需的 Surface
     * 尚未交付。
     *
     * 挂起而非丢弃：用户点了快捷方式必须有反应；也不得提前建（无 Surface 建不出
     * 会话）。若既不建也不记下，请求要么落空，要么滞留到用户下一次任意新建会话
     * 才生效——把一个无关的普通会话变成应急会话。
     */
    @Volatile private var pendingFailsafeSession = false

    /**
     * 待执行的会话切换请求（来自桌面通知点击）；0 表示无请求。
     *
     * 通知点击常把应用从后台拉起，那时 Surface 尚未交付——直接切换会因「surface null
     * or invalid」失败，用户点了通知却什么都没发生。故记下请求，待 Surface 就绪
     * 后再执行。
     */
    @Volatile private var pendingSessionSwitchId: Long = 0L

    /**
     * 切到指定会话；Surface 未就绪时记下请求，待 [startRuntime] 拿到可用 Surface
     * 后执行。
     *
     * @param freshIntent 本次 Intent 是否为新投递。Activity 重建会再次走 `onCreate`
     *   并带上同一个启动 Intent；据此再切一次会话，用户在重建前手动切走的会话会被
     *   拽回去。与 Failsafe Intent 同一纪律：重建时的重复投递不重放状态改写。
     */
    fun requestSessionSwitch(sessionId: Long, freshIntent: Boolean) {
        if (!freshIntent) {
            LogUtil.info(
                "TerminalViewModel",
                "requestSessionSwitch: stale launch intent, ignored (id=$sessionId)",
            )
            return
        }
        if (currentSurface?.isValid != true || surfaceWidth <= 0 || surfaceHeight <= 0) {
            pendingSessionSwitchId = sessionId
            LogUtil.info(
                "TerminalViewModel",
                "requestSessionSwitch: surface not ready; pending id=$sessionId",
            )
            return
        }
        pendingSessionSwitchId = 0L
        switchSession(sessionId)
    }

    /**
     * 外部 Failsafe Intent（应用快捷方式 / 第三方启动器）到达。
     *
     * 应用未启动时首个会话由 [startRuntime] → `runtime.start()` 建出，它读到 failsafe
     * 标记后即以 `/system/bin/sh` 启动；应用已启动时 `start()` 会因「已有会话」早退，
     * 故必须显式建一个新会话——否则快捷方式点了毫无反应，而标记滞留到用户下一次
     * 任意新建会话时才生效（把无关的普通会话变成应急会话）。
     *
     * @param freshIntent 本次 Intent 是新投递的。Activity 重建会再次走 `onCreate`
     *   并带上同一个启动 Intent；若据此再建会话，每次配置变更（密度、语言）都会
     *   多出一个应急会话。重建时运行期已有会话，故按 [decideFailsafeIntent] 忽略。
     */
    fun requestFailsafeSession(freshIntent: Boolean) {
        val hasSession = runtime.state.value.sessionIds.isNotEmpty()
        when (decideFailsafeIntent(hasSession = hasSession, freshIntent = freshIntent)) {
            FailsafeIntentAction.IGNORE -> {
                LogUtil.debug(
                    "TerminalViewModel",
                    "requestFailsafeSession: stale launch intent, ignored",
                )
            }

            FailsafeIntentAction.MARK_ONLY -> {
                // 冷启动：首个会话尚未建出，runtime.start() 会读到标记。
                runtime.requestFailsafeSession()
            }

            FailsafeIntentAction.MARK_AND_CREATE -> {
                runtime.requestFailsafeSession()
                pendingFailsafeSession = true
                createPendingFailsafeSession()
            }
        }
    }

    /**
     * Surface 就绪后补建挂起的 Failsafe 会话（见 [requestFailsafeSession]）。
     *
     * Surface 尚未交付时保持挂起：请求不得丢（用户点了快捷方式必须有反应），
     * 也不得提前建（无 Surface 建不出会话）。
     */
    private fun createPendingFailsafeSession() {
        if (!pendingFailsafeSession) return
        if (currentSurface?.isValid != true || surfaceWidth <= 0 || surfaceHeight <= 0) {
            LogUtil.info(
                "TerminalViewModel",
                "createPendingFailsafeSession: surface not ready; pending",
            )
            return
        }
        pendingFailsafeSession = false
        createSession()
    }

    fun startRuntime(surface: Surface?, width: Int, height: Int) {
        // 尺寸先于 Surface：currentSurface 的 setter 会补建挂起的 Failsafe 会话，
        // 而建会话需要尺寸就绪（见 [createPendingFailsafeSession]）。
        surfaceWidth = width
        surfaceHeight = height
        currentSurface = surface
        viewModelScope.launch(TerminalDispatchers.inputOutput) {
            runtime.start(surface, width, height)
            // 通知点击带来的切换请求：此时 Surface 已就绪、启动已完成。
            val pending = pendingSessionSwitchId
            if (pending != 0L) {
                pendingSessionSwitchId = 0L
                switchSession(pending)
            }
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
            // 不会闪现固定的 10sp 兜底值；缺省值只从 Repository 取，不在此重算。
            SettingsRepository.SettingsState(
                fontSize = settingsRepository.defaultFontSize,
                // 主题名留空：它们的缺省值来自原生主题目录（JNI），首帧只是占位，
                // 真值随流到达。不为此在 ViewModel 构造期强行加载原生库。
                themeName = "",
                dayThemeName = "",
                nightThemeName = "",
            ),
        )

    /** 系统可用字体族（枚举由 FontManager 负责）。 */
    val availableFonts: StateFlow<List<String>> = fontManager.availableFonts

    /** 原生当前默认字体族名（由 FontManager 发布）。 */
    val defaultFontName: StateFlow<String> = fontManager.defaultFontName

    /** 原生字体信息；null 表示原生尚未上报（由 FontManager 发布）。 */
    val fontInfo: StateFlow<FontInfo?> = fontManager.fontInfo

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
                            selectionManager.clampSelectionToGrid(
                                current.selection,
                                runtimeState.rows,
                                runtimeState.cols,
                            ),
                        )
                    }
                }
                // 运行期按升序发布（契约见 `RuntimeState.sessionIds`）：合并时保留已取回的
                // 标题/目录，排序只在 mergeSessions 一处发生——「谁负责排序」只有一个答案。
                val sortedIds = runtimeState.sessionIds
                val sessions = mergeSessions(_state.value.sessions, sortedIds)
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
                    // 用本轮的快照，不再二次读 runtime.state：两读之间会话可能已全部关闭，
                    // 那会让已发布的空列表又去应用一次字体。
                    if (sortedIds.isNotEmpty()) {
                        // 会话启动只应用已存字体（bridge 按会话持有）；列表枚举是
                        // 字体列表 UI 的事，不在此触发。
                        fontManager.applyStoredFontSettings()
                        // 字体查询是同步 JNI，不得在 Main.immediate 收集器上执行。
                        viewModelScope.launch(TerminalDispatchers.inputOutput) {
                            fontManager.publishFontState(runtime.bridge())
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
     * 清空设置内容，删除与用户数据（files 目录下的 home/usr）无关的设置与缓存数据，
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
                // 先经 DataStore 清空内容：它缓存着已读入的状态，只删文件的话
                // 内存态仍是旧值，下一次写入还会把旧值整份写回。
                // 清空失败按未清除记账：照旧提示「已清除」就是把假话当事实。
                val settingsReset =
                    runCatchingCancellable { settingsRepository.clearAllSettings() }
                        .onFailure { LogUtil.error("TerminalViewModel", "clear app data failed to reset settings", it) }
                        .isSuccess
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
                val survivors =
                    undeletedWatchedDirs + undeletedCacheEntries +
                        if (settingsReset) emptyList() else listOf("settings")
                // 重建必须无条件执行：删除与重建必须成对——目录一旦消失，进程级
                // DataStore 单例将因父目录缺失而写不进去，此后所有设置写入全失败，
                // 比不清除更糟。
                watchedDirs.forEach { context.getDir(it, Context.MODE_PRIVATE) }
                if (survivors.isNotEmpty()) {
                    LogUtil.error("TerminalViewModel", "clear app data incomplete, undeleted or unreadable: $survivors")
                    return@launch
                }
                withContext(TerminalDispatchers.main) {
                    Toast.makeText(
                        context,
                        context.getString(R.string.clear_app_data_done),
                        Toast.LENGTH_LONG,
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
     * 按 id 升序合并会话列表：已有条目连同标题与工作目录一起保留。
     *
     * 运行期收集器与 [createSession] 做的是同一件事。两处各写一份时，任一处漏掉
     * `previousById` 就会把其余会话当成新条目、清掉抽屉里已取回的标题与目录；
     * 排序也只在这里发生，避免「谁负责排序」有两处答案。
     */
    private fun mergeSessions(previous: List<SessionInfo>, ids: List<Long>): List<SessionInfo> {
        val previousById = previous.associateBy { it.sessionId }
        return ids.distinct().sorted().map { previousById[it] ?: SessionInfo(sessionId = it) }
    }

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

    // ── 主题转发（实现在 ThemeManager） ──

    fun setThemeName(name: String) = themeManager.setThemeName(name)

    fun setDayThemeName(name: String) = themeManager.setDayThemeName(name)

    fun setNightThemeName(name: String) = themeManager.setNightThemeName(name)

    fun setThemeMode(mode: String) = themeManager.setThemeMode(mode)

    fun setAppThemeMode(mode: String) = themeManager.setAppThemeMode(mode)

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
                        // 就并入了 newId（两者都跑在 IO 调度器上）——mergeSessions
                        // 去重使列表不含重复 id（否则 LazyColumn 键冲突：
                        // "Key N was already used"）。
                        val sessions = mergeSessions(current.sessions, current.sessions.map { it.sessionId } + newId)
                        val displayIndex = sessions.indexOfFirst { it.sessionId == newId } + 1
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
                if (exception is CancellationException) throw exception
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
                    if (exception is CancellationException) throw exception
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

    /**
     * 播放 `extra-keys` 的宏（`{macro: "CTRL f d"}`）。
     *
     * 有键码的普通记号走原生键编码器：修饰位与可打印键的**平台文本**一并下发，编码器
     * 据此折叠 Ctrl 字节、加 ESC 前缀（Alt）、并按 Shift 给出大写文本。只下键码不下
     * 文本的话，可打印键在编码器眼里「没有字面量」——它不会发出任何字节（上游
     * `key::Encoder` 的既有约束）。
     *
     * 没有键码的记号按上游 `TerminalView.inputCodePoint` 的语义直接写出：Ctrl 折叠每个
     * 码点，Alt 在每个码点前加 ESC，Shift 对字面量没有作用。
     *
     * 宏自带修饰位，不吃键栏上的粘滞 CTRL/ALT：两者叠加会让「Ctrl f」这类宏在用户
     * 恰好按着粘滞 Ctrl 时变成 Ctrl+Ctrl+f。
     */
    fun sendMacro(steps: List<MacroStep>) {
        val bridge = runtime.bridge() ?: return
        var modifiers = 0
        for (step in steps) {
            when (step) {
                is MacroStep.Modifier -> modifiers = modifiers or step.bit

                is MacroStep.Key -> {
                    sendMacroKey(bridge, step, modifiers)
                    // 修饰位只作用于其后第一个普通键（上游语义）。
                    modifiers = 0
                }

                is MacroStep.Text -> {
                    val bytes = MacroParser.encodeLiteral(step.text, modifiers)
                    if (bytes.isNotEmpty()) writeToPty(runtime.inputTargetSessionId, bytes)
                    modifiers = 0
                }
            }
        }
    }

    /** 宏里的一个普通键：键码 + 平台文本 → 编码器。 */
    private fun sendMacroKey(bridge: Bridge, step: MacroStep.Key, modifiers: Int) {
        // 文本只按 Shift 取：Ctrl 的字面量仍是未折叠的字母（折叠由编码器做），
        // Alt 的 ESC 前缀也由编码器加——文本若已带上这两种修饰，编码器就会折两次。
        val metaState = if (modifiers and KeyModifiers.SHIFT != 0) KeyEvent.META_SHIFT_ON else 0
        val text = macroCharacterMap.get(step.keyCode, metaState)
        val unshifted = macroCharacterMap.get(step.keyCode, 0)
        if (!bridge.processKeyEvent(step.keyCode, modifiers, KeyEvent.ACTION_DOWN, text, unshifted)) {
            LogUtil.warn("TerminalViewModel", "macro key '${step.token}' produced no bytes")
        }
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
                if (exception is CancellationException) throw exception
                LogUtil.error("TerminalViewModel", "closeSession failed for id=$sessionId", exception)
                return@launch
            }
            withContext(TerminalDispatchers.main) {
                _state.update { current ->
                    val remaining = current.sessions.filter { it.sessionId != sessionId }.sortedBy { it.sessionId }
                    if (remaining.isEmpty()) {
                        current.copy(
                            isRunning = false,
                            sessions = emptyList(),
                            activeSessionId = 0L,
                            selection = SelectionState(),
                        )
                    } else {
                        val newActive =
                            if (current.activeSessionId == sessionId) {
                                // 与运行期同一条规则（`remaining.keys.sorted().last()`）：
                                // 接替的是最大 id。两侧各选各的会让 UI 高亮的会话与真正
                                // 接收输入的会话分叉，敲键就发往未显示的会话。
                                remaining.last().sessionId
                            } else {
                                current.activeSessionId
                            }
                        val newActiveIndex = remaining.indexOfFirst { it.sessionId == newActive }
                        current.copy(
                            sessions = remaining,
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
                if (exception is CancellationException) throw exception
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
