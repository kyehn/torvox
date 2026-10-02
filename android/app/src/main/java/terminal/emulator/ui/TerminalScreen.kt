// 文件内多处 LaunchedEffect 等非组合作用域必须经 context 取资源，stringResource 不可用。
@file:Suppress("LocalContextGetResourceValueCall")

package terminal.emulator.ui

import android.annotation.SuppressLint
import android.os.Handler
import android.os.Looper
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.TerminalViewModel
import terminal.emulator.bridge.Bridge
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.computeImeSurfaceShift
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName
import kotlin.math.max
import kotlin.math.min

private const val FONT_SIZE_MIN = 14f
private const val FONT_SIZE_MAX = 48f

// toggleKeyboard lambda 在抽屉完全关闭时已跳过延迟路径；
// 抽屉打开时，50ms 足以让遮罩轻击在输入法与关闭动画竞争前生效。
// 原先的 250ms 会被感知为输入延迟。
private const val IME_TOGGLE_DELAY_MS = 50L

// 「先平移后重排」的输入法混合方案（零重组）：布置阶段的偏移直接读取 WindowInsets，
// 无需 Compose 弹簧——系统的 WindowInsetsAnimation 已能平滑插值。
// 稳定判定为 3 个稳定帧 × 16ms = 48ms，与 TerminalSurface 的防抖一致。
private const val IME_SETTLE_FRAMES = 3
private const val IME_POLL_INTERVAL_MS = 16L

/** 位移稳定后的空闲轮询间隔：动画结束后无需逐帧跟随，降低常驻唤醒。 */
private const val IME_IDLE_POLL_INTERVAL_MS = 200L

/** 搜索查询串长度上限：与原生 `MAX_SEARCH_QUERY_CHARS` 同值，超长查询原生直接无命中。 */
private const val SEARCH_QUERY_MAX_LENGTH = 128

/** 终端内文本搜索的合并状态，取代原先 6 个独立的 remember 变量。 */
private data class SearchState(
    val query: String = "",
    val results: List<SearchResult> = emptyList(),
    val currentIndex: Int = 0,
    val caseSensitive: Boolean = false,
    val previousQuery: String = "",
    val highlightsActive: Boolean = false,
) {
    val hasResults: Boolean
        get() = results.isNotEmpty()

    val resultCount: Int
        get() = results.size

    val currentMatch: SearchResult?
        get() = results.getOrNull(currentIndex)
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
// 仅简体中文：无复数形态，plurals 仅 other 分支生效，无需本地化计数修饰。
@SuppressLint("ArgInFormattedQuantityStringRes")
@Composable
fun TerminalScreen(
    modifier: Modifier = Modifier,
    viewModel: TerminalViewModel = hiltViewModel(),
    onSettings: () -> Unit = {},
    isOverlayVisible: Boolean = false,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    // 终端内容 Box 的高度（下方的上下文菜单按相对此 Box 的偏移定位
    // ——菜单必须按 BOX 高度而非全屏高度放置，
    // 否则「全选」选区（selBottom == box 高度）会把菜单推到工具栏之下的屏幕外）。
    val viewModelThemeMode = settings.themeMode
    val viewModelThemeName = settings.themeName
    val viewModelDayThemeName = settings.dayThemeName
    val viewModelNightThemeName = settings.nightThemeName
    val runtimeState by viewModel.runtime.state.collectAsStateWithLifecycle()
    val isSettingsDark =
        resolveAppDarkMode(settings.appThemeMode, androidx.compose.foundation.isSystemInDarkTheme())
    val resolvedTerminalTheme =
        BuiltInThemes.byName(
            resolveTerminalThemeName(
                mode = viewModelThemeMode,
                fixedName = viewModelThemeName,
                dayName = viewModelDayThemeName,
                nightName = viewModelNightThemeName,
                isDark = isSettingsDark,
            ),
        )
    val terminalBackground = resolvedTerminalTheme.background
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var searchJob by remember { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    // 搜索查询的 150ms 真实防抖。SearchDebouncer 把快速连击合并为静默期后的
    // 一次 performSearch，而不是每次击键都触发一次全回滚搜索。
    // 生产调度器跑在主 Looper 上（TerminalSurface 回调在主线程）；
    // 单元测试使用假调度器。
    val searchDebouncer = remember {
        SearchDebouncer(
            debounceMillis = 150L,
            scheduler = HandlerDebounceScheduler(Handler(Looper.getMainLooper())),
        )
    }
    val context = androidx.compose.ui.platform.LocalContext.current
    var showTextSearch by remember { mutableStateOf(false) }
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    val view = LocalView.current
    val surfaceRef = remember { mutableStateOf<TerminalSurface?>(null) }
    // 切换软键盘（termux 的 KEYBOARD 键）：供会话抽屉的键盘按钮
    // 与自定义工具栏布局中的 KEYBOARD 附加键使用。
    // 在 Raw 键盘模式下为空操作（无可显示/隐藏的输入法）。
    // 可见性在轻点时从已挂载的 window insets 同步读取
    // ——绝不用 TerminalSurface.lastImeBottom：对于托管在 Compose AndroidView 中的
    // SurfaceView，其 SurfaceView.onApplyWindowInsets 回调不可靠，
    // 会使 imeVisible 永远陈旧（false），把切换退化为只能显示。
    val toggleKeyboard: () -> Unit = {
        if (state.keyboardMode != terminal.emulator.input.KeyboardMode.Raw) {
            val inputMethodManager =
                context.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                    as android.view.inputmethod.InputMethodManager
            val imeCurrentlyVisible =
                view.rootWindowInsets?.isVisible(android.view.WindowInsets.Type.ime()) == true
            if (imeCurrentlyVisible) {
                inputMethodManager.hideSoftInputFromWindow(view.windowToken, 0)
            } else {
                view.requestFocus()
                // 此延迟只为让会话抽屉的关闭动画稳定下来
                // ——在遮罩仍在动画时触发会与之竞争。
                // 抽屉完全关闭时无可等待，立即显示。
                val deferForDrawer = drawerState.isOpen || drawerState.isAnimationRunning
                if (!deferForDrawer) {
                    view.windowInsetsController?.show(
                        android.view.WindowInsets.Type.ime(),
                    )
                } else {
                    // 用户手势之外的 SHOW_IMPLICIT 在 Android 12+ 上会被静默拒绝
                    // （输入法可见性需要受信任的手势或窗口焦点信任），
                    // 这使抽屉的键盘按钮在显示方向形同虚设。
                    // 改用与终端轻点相同的 WindowInsetsController 路径（已证实能显示）；
                    // 它不受手势限制。
                    view.postDelayed(
                        {
                            view.windowInsetsController?.show(
                                android.view.WindowInsets.Type.ime(),
                            )
                        },
                        IME_TOGGLE_DELAY_MS,
                    )
                }
            }
        }
    }
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) {
                // 组合顶层的 LocalView.current 是 AndroidComposeView 而非 TerminalSurface；
                // 故使用从 AndroidView 工厂捕获的 surfaceRef。
                surfaceRef.value?.finishComposing()
                // 后台时停止渲染线程：切应用不会调用 surfaceDestroyed（Surface 被保留），
                // 不做此步线程会继续在已被系统回收的 BufferQueue 上取帧
                // → ERROR_SURFACE_LOST_KHR → 返回后永久黑屏（模拟器已验证）。
                viewModel.runtime.setRenderPaused(true)
                viewModel.runtime.pauseRendering()
                val inputMethodManager =
                    context.getSystemService(
                        android.content.Context.INPUT_METHOD_SERVICE,
                    ) as android.view.inputmethod.InputMethodManager
                inputMethodManager.hideSoftInputFromWindow(view.windowToken, 0)
            } else if (event == Lifecycle.Event.ON_RESUME) {
                val surface = surfaceRef.value
                // 切应用连续性：若 Surface 被保留（无 surfaceDestroyed），
                // render_paused 仍为 true（来自 onSurfaceDestroyed 或此前的暂停），
                // 而 surfaceCreated 从未被调用以清除它 → 黑屏。
                // Surface 已挂载且已定尺寸时立即清除；仅在 Surface 尚未就绪
                // （与布局竞争）时延迟 200ms。
                if (
                    surface != null && surface.isAttachedToWindow && surface.width > 0 && surface.height > 0
                ) {
                    // 即使 Surface 对象存活（无 surfaceDestroyed），
                    // 系统也可能在后台回收其 BufferQueue：
                    // 重新 attach 会去重配一个已死的 Surface → 永远 ERROR_SURFACE_LOST_KHR。
                    // 故先丢弃它，使 attach 走慢路径并从（此时又有效的）
                    // ANativeWindow 重建交换链。使用 holder 的实时 Surface，
                    // 而非缓存的 currentSurface（回收后其 isValid 恒为 false）。
                    val bridge = viewModel.runtime.bridge()
                    val holderSurface = surface.holder?.surface
                    if (bridge != null && holderSurface != null && holderSurface.isValid) {
                        viewModel.currentSurface = holderSurface
                        bridge.releaseGpuSurface()
                        bridge.attachSurface(holderSurface, surface.width, surface.height)
                        viewModel.runtime.setRenderPaused(false)
                        viewModel.runtime.resumeRendering()
                        viewModel.runtime.forceRender()
                    } else {
                        // holder 尚未有效（系统仍在恢复 BufferQueue）：
                        // 持续重试直到有效，然后重建。
                        // 此刻绝不要解除暂停——在已死的 Surface 上出帧会永远失败。
                        surface.postDelayedSurfaceRecreate(viewModel)
                    }
                } else {
                    surface?.postDelayedUnpause(200L)
                }
                view.requestFocus()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    BackHandler(enabled = drawerState.isOpen) {
        scope.launch { drawerState.close() }
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet(
                drawerContainerColor = MaterialTheme.colorScheme.surface,
            ) {
                SessionDrawer(
                    sessions = state.sessions,
                    activeSessionId = state.activeSessionId,
                    onSwitchSession = { viewModel.switchSession(it) },
                    onCloseSession = { viewModel.closeSession(it) },
                    onAddSession = { viewModel.createSession() },
                    onSettings = {
                        scope.launch { drawerState.close() }
                        onSettings()
                    },
                    onSearch = {
                        showTextSearch = true
                        surfaceRef.value?.searchActive = true
                    },
                    onKeyboardToggle = toggleKeyboard,
                    onResetTerminal = { viewModel.resetActiveTerminal() },
                    onClose = {
                        scope.launch { drawerState.close() }
                    },
                )
            }
        },
        modifier = modifier,
    ) {
        val snackbarHostState = remember { SnackbarHostState() }

        Box(
            modifier =
            Modifier.fillMaxSize()
                .testTag("TerminalScreen")
                .background(terminalBackground)
                .statusBarsPadding()
                .navigationBarsPadding()
                // 裁到安全区内：输入法位移把终端整体上移，位移量可达整个键盘高度，
                // 而 Compose 默认不裁剪，越过 statusBarsPadding 的内容会直接画到
                // 状态栏底下——时钟与终端行叠字，即输入法弹出时的内容重叠。
                // 置于两个内边距之后，裁剪矩形即安全区本身，位移内容被裁在区内而非压到状态栏。
                .clipToBounds(),
        ) {
            LaunchedEffect(drawerState.isOpen) {
                surfaceRef.value?.drawerOpen = drawerState.isOpen
                if (drawerState.isOpen) {
                    // 后台会话标题变化不产生状态事件，抽屉打开即强制刷新，否则显示过时标题。
                    viewModel.refreshSessionMetas(force = true)
                }
            }
            val selection = state.selection
            val selectionActive = selection.active && selection.start != null && selection.end != null

            // 使用旧的 View.startActionMode(Callback) 时，系统不会拦截返回键
            // 来结束 ActionMode（只有 TYPE_FLOATING 模式才会）；因此返回键会落到
            // Activity，在选区激活时退出应用（模拟器已验证）。
            // 故在选区激活时拦截返回键并先结束它——ghostty-android 的
            // onDestroyActionMode 会清除选区；Termux 的首次返回关闭工具栏。
            // 抽屉自身的 BackHandler 优先（它注册得更早，而 LIFO 使我们的先跑，
            // 故把我们的门控在「抽屉已关闭」上：
            // 抽屉打开时的返回键关闭抽屉，绝不能清除选区）。
            BackHandler(enabled = selectionActive && !drawerState.isOpen) {
                viewModel.clearSelection()
                surfaceRef.value?.hideSelectionMenu()
            }

            // 合并的文本搜索状态
            var searchState by remember { mutableStateOf(SearchState()) }

            // 视口滚动偏移的 Compose 可观察镜像：搜索高亮按绝对行→视口行换算绘制，
            // 换算依赖滚动偏移。直接读 Surface 的普通方法拿不到重组通知，滚动后
            // 高亮会整体停在旧行；此处由所有改变可见偏移的路径（手势、程序化滚动、
            // 会话切换与输出引起的贴底复位）同步。
            val viewportScrollOffset = remember { androidx.compose.runtime.mutableIntStateOf(0) }

            LaunchedEffect(state.activeSessionId) {
                showTextSearch = false
                searchState = SearchState()
                surfaceRef.value?.searchActive = false
                // Surface 持有一个用于选区坐标计算的私有 scrollOffset；
                // 切换时它必须跟随会话自身的偏移，
                // 否则切换后的首个手势会算出错误的网格行。
                surfaceRef.value?.resetScrollOffset()
                viewportScrollOffset.intValue = viewModel.runtime.activeSessionScrollOffset()
            }

            // 程序化滚动复位（由输入驱动的贴底）：重同步 Surface 的私有偏移，
            // 使选区/拖动的坐标计算与运行时视口保持一致。
            LaunchedEffect(state.scrollEpoch) {
                if (state.scrollEpoch > 0L) {
                    surfaceRef.value?.resetScrollOffset()
                    viewportScrollOffset.intValue = viewModel.runtime.activeSessionScrollOffset()
                }
            }

            // 新输出驱动的自动贴底（渲染线程复位）：同步搜索高亮的偏移镜像，
            // 否则搜索中新输出到达后高亮按旧偏移错位绘制。
            LaunchedEffect(runtimeState.scrollResetEpoch) {
                if (runtimeState.scrollResetEpoch > 0L) {
                    viewportScrollOffset.intValue = viewModel.runtime.activeSessionScrollOffset()
                }
            }

            fun scrollToMatchIfNeeded(match: SearchResult) {
                val surface = surfaceRef.value ?: return
                val visibleRows = surface.getRows()
                val scrollbackLen = surface.getMaxScrollOffset()
                val scrollOffset = surface.getScrollOffset()
                val firstVisibleRow = scrollbackLen - scrollOffset
                val lastVisibleRow = firstVisibleRow + visibleRows - 1
                if (match.lineIndex !in firstVisibleRow..lastVisibleRow) {
                    val centeredRow = (match.lineIndex - visibleRows / 2).coerceAtLeast(0)
                    surface.scrollToRow(centeredRow)
                }
            }

            suspend fun performSearch() {
                val query = searchState.query
                if (query.isEmpty()) {
                    searchState = searchState.copy(results = emptyList())
                    return
                }
                surfaceRef.value?.finishComposing()
                val bridge =
                    viewModel.runtime.bridge()
                        ?: run {
                            searchState = searchState.copy(results = emptyList())
                            return
                        }
                val effectiveCaseSensitive =
                    searchState.caseSensitive || query.any { it.isUpperCase() }
                val matches =
                    bridge.searchAllInScrollback(query, effectiveCaseSensitive)
                        ?: run {
                            searchState = searchState.copy(results = emptyList())
                            return
                        }
                val results = matches.map { (row, startCol, endCol) ->
                    SearchResult(lineIndex = row, startIndex = startCol, endIndex = endCol)
                }
                // 收窄判定：GNOME Console（kgx）用 g_strrstr() 检查 last_search
                // 字符串是否*包含*当前查询——而非仅做前缀匹配。
                // 这使收窄在用户从搜索串中部或末尾删字时也能生效，
                // 而不只是删掉末尾字符时。
                val isNarrowing = SearchResult.isNarrowingDown(query, searchState.previousQuery)
                val newIndex =
                    if (isNarrowing && results.isNotEmpty()) {
                        searchState.currentIndex.coerceIn(0, results.size - 1)
                    } else {
                        0
                    }
                searchState =
                    searchState.copy(
                        results = results,
                        currentIndex = newIndex,
                        previousQuery = query,
                    )
                if (results.isNotEmpty()) {
                    scrollToMatchIfNeeded(results[newIndex])
                }
            }

            LaunchedEffect(searchState.caseSensitive) {
                if (searchState.query.isNotEmpty()) {
                    searchJob?.cancel()
                    searchJob = scope.launch { performSearch() }
                }
            }

            // IME 跟随：单一位移不重排。
            // 终端 Surface 与键栏同处一个平移容器（TerminalContent），整容器按键盘高度
            // 平移，双位移差拍（内容重叠/持续闪烁/键栏被输入法遮住）在结构上不可能发生。
            // Surface 尺寸永不变化，故不触发交换链重建与网格重排。
            //
            // insets 有两个来源，**按较大者合成**而非后写覆盖：
            //  1) [WindowImeBottomPx] 叶节点——动画期间逐帧给出真实键盘高度
            //     （实测 0→772→818→820 后静止）。
            //  2) 轮询 rootWindowInsets——视图系统通道。Compose 订阅并非在所有环境都
            //     收到更新（实测仪器化运行中叶节点恒为 0，而 rootWindowInsets 已报出
            //     真实键盘高度），故此路必须保留。
            //
            // 早期实现让两路**后写覆盖**同一状态，视图通道在 insets dispatch 遍历中
            // 读到尚未更新的 ime=0，写入布局状态又触发新一轮 dispatch，实测自激振荡
            // 506 次，键栏在「键盘上方」与「键盘后方」之间来回跳，即持续闪烁的根因。
            // 取较大者后任一方的 0 都压不掉对方的真实值，振荡在结构上不可能复现；
            // 两侧同时归零（键盘收起）时位移同样归零。
            val imeLeafPx = remember { androidx.compose.runtime.mutableIntStateOf(0) }
            WindowImeBottomPx { imeLeafPx.intValue = it }
            // 视图通道：轮询 rootWindowInsets，不监听 dispatch。
            //
            // 监听 dispatch 两次踩坑：其一，在 dispatch 遍历内写布局状态会触发新一轮
            // dispatch，读到的 ime 在 0 与真实高度间往复，形成自激回路（实测常驻 46% CPU，
            // 并使 UiAutomation.takeScreenshot() 永不稳定）；其二，改为「静止后才采纳」
            // 后又被连续 dispatch 反复取消计时而饿死，位移时有时无。
            // 轮询只读框架已算好的 insets，不回灌 dispatch，两条问题都不成立；
            // 且 rootWindowInsets 本身可靠——实测仪器化环境下 Compose 叶节点恒为 0，
            // rootWindowInsets 仍给出真实键盘高度。
            //
            // 值变化时按动画帧率轮询，稳定后退到空闲间隔，避免常驻高频唤醒。
            val imeViewPx = remember { androidx.compose.runtime.mutableIntStateOf(0) }
            val windowRoot = LocalView.current.rootView
            LaunchedEffect(Unit) {
                var lastSeen = -1
                while (true) {
                    // 取窗口根视图而非 surfaceRef：AndroidView 可能重建视图，
                    // surfaceRef 里那一份会脱离窗口、rootWindowInsets 恒为 0
                    // （实测仪器化环境下轮询只读到一次 0，位移因此从未发生）。
                    val insets = windowRoot?.rootWindowInsets
                    val imeBottom = insets?.getInsets(android.view.WindowInsets.Type.ime())?.bottom ?: 0
                    val navigationBottom =
                        insets?.getInsets(android.view.WindowInsets.Type.navigationBars())?.bottom ?: 0
                    val bottom = max(imeBottom - navigationBottom, 0)
                    val changed = bottom != lastSeen
                    if (changed) {
                        lastSeen = bottom
                        imeViewPx.intValue = bottom
                    }
                    kotlinx.coroutines.delay(
                        if (changed) IME_POLL_INTERVAL_MS else IME_IDLE_POLL_INTERVAL_MS,
                    )
                }
            }
            val settledImePx = remember { androidx.compose.runtime.mutableIntStateOf(0) }
            // 定居节流：值停止变化 IME_SETTLE_FRAMES×轮询间隔后锁定 settled 值。
            // 位移本身直接读合成值（每帧即跟随 live 值），不被此节流阻塞——
            // 否则动画期间终端与键栏冻结、定居后跳变（违反逐帧跟随）。
            LaunchedEffect(Unit) {
                snapshotFlow { max(imeLeafPx.intValue, imeViewPx.intValue) }
                    .distinctUntilChanged()
                    .collectLatest { imeBottom ->
                        delay(IME_POLL_INTERVAL_MS * IME_SETTLE_FRAMES)
                        if (imeBottom != settledImePx.intValue) {
                            settledImePx.intValue = imeBottom
                            surfaceRef.value?.onImeSettled(imeBottom)
                        }
                    }
            }
            // 内容下沿：视口最后一个有内容的行（渲染线程随每帧单元数据发布，
            // 空闲帧同样更新，故输入法动画与定居后都不读到陈旧值）。
            //
            // 只被位移 lambda 在 placement 期读取，故内容变化/光标移动**不**触发
            // 主组合重组（与 ime 状态同构的处理）。
            val lastContentRow = remember {
                androidx.compose.runtime.mutableIntStateOf(Bridge.LAST_CONTENT_ROW_NONE)
            }
            val debugShiftHolder = remember { IntArray(2) }
            val runtimeForContent = viewModel.runtime
            LaunchedEffect(runtimeForContent) {
                snapshotFlow { runtimeForContent.lastContentRowFlow.value }
                    .distinctUntilChanged()
                    .collect { lastContentRow.intValue = it }
            }
            // 位移容器不再整体平移：终端 Surface 与键栏各自持有自己的位移量，
            // 但两者都只读上面那一个合成 ime 状态、并在同一帧 placement 中求值——
            // 唯一位移来源不变（双位移源的历史振荡因此不会复现），而平移量可以
            // 按内容裁剪：网格自顶端锚定渲染，键盘遮住的是网格末尾行，
            // 按整块键盘高度上移会把稀疏会话的提示符推出屏幕上边界（终端区全空）。
            //
            // Surface 尺寸全程不变，故网格不重排、无 SIGWINCH。
            Box(
                modifier =
                Modifier.fillMaxSize()
                    .testTag("TerminalContent"),
            ) {
                // 终端 Surface 占满整块高度：键栏覆盖其底部，而网格已按同一口径预留
                // 键栏高度（见 TerminalSurface.ResizeManager），故 rows/cols 不受键栏位移影响。
                Box(
                    modifier =
                    Modifier.fillMaxSize().layout { measurable, constraints ->
                        val placeable = measurable.measure(constraints)
                        val cellHeightPx = runtimeForContent.cellHeight
                        val contentRow = lastContentRow.intValue
                        val contentBottomPx =
                            if (contentRow < 0) 0 else (contentRow + 1) * cellHeightPx.toInt()
                        val shift =
                            computeImeSurfaceShift(
                                contentBottomPx = contentBottomPx,
                                surfaceHeightPx = placeable.height,
                                modifierBarHeightPx = runtimeForContent.modifierBarHeightPx,
                                imeBottomPx = max(imeLeafPx.intValue, imeViewPx.intValue),
                            )
                        val holder = debugShiftHolder
                        if (holder[0] != shift || holder[1] != placeable.height) {
                            holder[0] = shift
                            holder[1] = placeable.height
                            LogUtil.d(
                                "TerminalScreen",
                                "DEBUG_IME shift=$shift H=${placeable.height} " +
                                    "bar=${runtimeForContent.modifierBarHeightPx} " +
                                    "ime=${max(imeLeafPx.intValue, imeViewPx.intValue)} " +
                                    "row=$contentRow bottom=$contentBottomPx",
                            )
                        }
                        layout(placeable.width, placeable.height) { placeable.placeRelative(0, -shift) }
                    },
                ) {
                    AndroidView(
                        factory = { context ->
                            terminal.emulator.ui
                                .TerminalSurface(context)
                                .apply { setTag("TerminalSurfaceView") }
                                .also { surface ->
                                    surfaceRef.value = surface
                                    surface.attachViewModel(viewModel)
                                    surface.onScrollChanged = { offset ->
                                        viewportScrollOffset.intValue = offset
                                        viewModel.runtime.setScrollOffset(offset)
                                    }
                                    surface.onScrollingStateChanged = { isScrolling ->
                                        viewModel.runtime.setScrollActive(isScrolling)
                                    }
                                }
                                .apply {
                                    setDimensions(runtimeState.rows, runtimeState.cols)
                                    onCopyRequested = { text ->
                                        scope.launch {
                                            snackbarHostState.currentSnackbarData?.dismiss()
                                            snackbarHostState.showSnackbar(
                                                message =
                                                context.resources.getQuantityString(
                                                    R.plurals.copied_chars,
                                                    text.length,
                                                    text.length,
                                                ),
                                                duration = SnackbarDuration.Short,
                                            )
                                        }
                                    }
                                    onPasteRequested = {
                                        val count = viewModel.pasteFromClipboard()
                                        if (count > 0) {
                                            scope.launch {
                                                snackbarHostState.currentSnackbarData?.dismiss()
                                                snackbarHostState.showSnackbar(
                                                    message =
                                                    context.resources.getQuantityString(
                                                        R.plurals.pasted_chars,
                                                        count,
                                                        count,
                                                    ),
                                                    duration = SnackbarDuration.Short,
                                                )
                                            }
                                        }
                                    }
                                    onZoomChanged = { sizeSp ->
                                        // ⑥ 双指缩放终结：持久化稳定尺寸并执行完整应用
                                        // （单次网格重排）。
                                        viewModel.setFontSize(sizeSp.coerceIn(FONT_SIZE_MIN, FONT_SIZE_MAX))
                                    }
                                    onZoomPreview = { sizeSp ->
                                        // 手势预览只推字形度量不重算网格，网格只在
                                        // finalize 重算一次，避免手势期间中间态撕裂。
                                        viewModel.runtime.setFontSizePreview(sizeSp)
                                    }
                                    post {
                                        requestFocus()
                                    }
                                }
                        },
                        update = { surface ->
                            surface.touchEnabled = !isOverlayVisible
                            // 仅在终端网格尺寸真正变化时（resize / 字体变化）才重新布局。
                            // AndroidView 的 update 块在 TerminalScreen 的每次重组上都会运行，
                            // 此处无条件 requestLayout() 会迫使每次选区拖动与滚动事件
                            // 都走一遍完整的 View 布局流程——UI 卡顿的一个主要来源。
                            if (
                                runtimeState.rows > 0 &&
                                runtimeState.cols > 0 &&
                                (
                                    surface.getRows() != runtimeState.rows ||
                                        surface.getCols() != runtimeState.cols
                                    )
                            ) {
                                surface.setDimensions(runtimeState.rows, runtimeState.cols)
                                surface.requestLayout()
                            }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )

                    if (selectionActive) {
                        val selStart = selection.start
                        val selEnd = selection.end
                        val loRow = min(selStart.row, selEnd.row)
                        val hiRow = max(selStart.row, selEnd.row)
                        val loCol: Int
                        val hiCol: Int
                        if (selStart.row <= selEnd.row) {
                            loCol = selStart.col
                            hiCol = selEnd.col
                        } else {
                            loCol = selEnd.col
                            hiCol = selStart.col
                        }
                        val themeAccent =
                            if (state.selectionAccent != 0) {
                                Color(state.selectionAccent)
                            } else {
                                resolvedTerminalTheme.foreground
                            }

                        val themeAccentArgb = themeAccent.toArgb()

                        // 以整个选区状态为 key 的单一 effect：
                        // key 变化时先取消正在运行的 effect，
                        // 故未改变锚点单元格就结束的拖动不会把手柄留在隐藏状态
                        // （此前拆分的显示/隐藏 effect 会在软件渲染器上乱序执行，
                        // 在显示之后又隐藏）。
                        LaunchedEffect(
                            selectionActive,
                            selection.dragging,
                            loRow,
                            loCol,
                            hiRow,
                            hiCol,
                            themeAccentArgb,
                        ) {
                            // 手柄拖动进行中时，单一覆盖层拥有触摸事件流
                            // 并在进程内重定位手柄
                            // ——此处的 hideSelectionHandles() 会在手指底下
                            // dismiss 窗口，从而在首次跨单元格移动后杀死手势。
                            // 拖动期间刻意为空操作。菜单另行经 menuDismissed 隐藏。
                            if (!selection.dragging) {
                                surfaceRef.value?.showSelectionHandles(loRow, loCol, hiRow, hiCol, themeAccentArgb)
                            }
                        }
                    } else {
                        LaunchedEffect(selectionActive) {
                            surfaceRef.value?.hideSelectionHandles()
                        }
                    }

                    // ── 选区上下文菜单（PopupWindow） ──
                    // 菜单必须是 PopupWindow 而非 Compose 覆盖层
                    // ——终端是 SurfaceView，其 surface 在整个终端区域开了孔，
                    // 会遮住任何窗口内的 Compose 绘制。
                    // PopupWindow 是独立的系统窗口，渲染在其之上。
                    val menuSurface = surfaceRef.value
                    if (menuSurface != null && selectionActive && !selection.dragging) {
                        val menuVisible = !selection.menuDismissed
                        if (menuVisible) {
                            val themeAccentArgb =
                                if (state.selectionAccent != 0) {
                                    Color(state.selectionAccent)
                                } else {
                                    resolvedTerminalTheme.foreground
                                }
                                    .toArgb()
                            // 选择菜单走 Surface 侧 PopupWindow 定位与绘制，不用系统 ActionMode。
                            LaunchedEffect(selection.pasteOnly, selection.menuDismissed) {
                                if (selection.menuDismissed) {
                                    menuSurface.hideSelectionMenu()
                                } else {
                                    menuSurface.showSelectionMenu(selection.pasteOnly)
                                }
                            }
                        } else {
                            LaunchedEffect(Unit) { menuSurface.hideSelectionMenu() }
                        }
                    } else {
                        LaunchedEffect(selectionActive) {
                            menuSurface?.hideSelectionMenu()
                        }
                    }

                    // 搜索高亮绘制必须作为副作用运行，而不能在组合期间内联：
                    // 它会调入原生（bridge.render）并改写 searchState，
                    // 否则会在每次重组时重复执行，并在组合期间触发状态写入。
                    LaunchedEffect(
                        showTextSearch,
                        searchState.hasResults,
                        searchState.resultCount,
                        searchState.currentIndex,
                        searchState.results,
                        viewportScrollOffset.intValue,
                        surfaceRef.value?.getMaxScrollOffset(),
                        resolvedTerminalTheme.foreground,
                        resolvedTerminalTheme.selectionBackground,
                    ) {
                        if (showTextSearch && searchState.hasResults) {
                            val surface = surfaceRef.value
                            if (surface != null) {
                                val rows = surface.getRows()
                                val scrollbackCount = surface.getMaxScrollOffset()
                                val scrollOffset = viewportScrollOffset.intValue
                                val themeForeground = resolvedTerminalTheme.foreground
                                val themeSelectionBackground = resolvedTerminalTheme.selectionBackground

                                val highlightBuffer = java.io.ByteArrayOutputStream()
                                fun writeI32(value: Int) {
                                    highlightBuffer.write(value and 0xFF)
                                    highlightBuffer.write((value ushr 8) and 0xFF)
                                    highlightBuffer.write((value ushr 16) and 0xFF)
                                    highlightBuffer.write((value ushr 24) and 0xFF)
                                }
                                fun writeByte(value: Byte) {
                                    highlightBuffer.write(value.toInt())
                                }
                                writeI32(searchState.resultCount)
                                for ((index, match) in searchState.results.withIndex()) {
                                    val gridRow = match.lineIndex - scrollbackCount + scrollOffset
                                    if (gridRow < 0 || gridRow >= rows) continue
                                    val isCurrent = index == searchState.currentIndex
                                    writeI32(gridRow)
                                    writeI32(match.startIndex)
                                    writeI32(match.endIndex.coerceAtLeast(match.startIndex + 1))
                                    if (isCurrent) {
                                        // 当前命中项：完全不透明的主题前景色。
                                        // alpha >= 128 使 Rust 渲染器交换前景/背景
                                        // （反色）并把不透明色混合到背景上，
                                        // 使当前命中项毫无疑问地突出。
                                        // 见 SearchHighlightColors.CURRENT_MATCH_ALPHA
                                        // 与 native/src/render/tests.rs 的生产值测试。
                                        writeByte((themeForeground.red * 255).toInt().toByte())
                                        writeByte((themeForeground.green * 255).toInt().toByte())
                                        writeByte((themeForeground.blue * 255).toInt().toByte())
                                        writeByte(SearchHighlightColors.CURRENT_MATCH_ALPHA.toByte())
                                    } else {
                                        // 其余命中项：低于 128 反色阈值的
                                        // selection_background 着色——可见覆盖层，不反色。
                                        // 见 SearchHighlightColors.OTHER_MATCH_ALPHA。
                                        writeByte((themeSelectionBackground.red * 255).toInt().toByte())
                                        writeByte((themeSelectionBackground.green * 255).toInt().toByte())
                                        writeByte((themeSelectionBackground.blue * 255).toInt().toByte())
                                        writeByte(SearchHighlightColors.OTHER_MATCH_ALPHA.toByte())
                                    }
                                }
                                val highlightBytes = highlightBuffer.toByteArray()
                                // 单次调用：surface.setSearchHighlights 内部会调用
                                // bridge.setSearchHighlights + bridge.render
                                surface.setSearchHighlights(highlightBytes)
                                searchState = searchState.copy(highlightsActive = true)
                            }
                        } else if (searchState.highlightsActive) {
                            surfaceRef.value?.clearSearchHighlights()
                            searchState = searchState.copy(highlightsActive = false)
                        }
                    }
                }

                // 键栏覆盖在 Surface 底部（网格已预留其高度），按整块键盘高度上移——
                // 恒位于输入法上方而不被遮挡（与 Surface 同一个 ime 状态、同一次 placement）。
                Box(
                    modifier =
                    Modifier.align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .offset { IntOffset(0, -max(imeLeafPx.intValue, imeViewPx.intValue)) }
                        .background(resolvedTerminalTheme.background)
                        .testTag("ModifierBarOverlay"),
                ) {
                    // 底部栏——位于终端之下、输入法之上
                    if (showTextSearch) {
                        TextSearchBar(
                            query = searchState.query,
                            onQueryChange = { newQuery ->
                                // 匹配文本长度须受限（DESIGN 修饰键栏节）：查询串直接送入
                                // 原生全回滚区扫描，过长会使单次搜索耗时不可预测。
                                searchState = searchState.copy(query = newQuery.take(SEARCH_QUERY_MAX_LENGTH))
                                searchJob?.cancel()
                                // 防抖：连续击键在静默 150ms 后合并为一次 performSearch
                                // （termlib / ghostty-android 做法）。待执行的搜索由
                                // debouncer 取消，而非每次击键都新起协程。
                                searchDebouncer.submit {
                                    searchJob = scope.launch { performSearch() }
                                }
                            },
                            resultCount = searchState.resultCount,
                            currentResultIndex = searchState.currentIndex,
                            onPrevious = {
                                if (searchState.hasResults) {
                                    val newIndex =
                                        SearchResult.previousIndex(
                                            searchState.currentIndex,
                                            searchState.resultCount,
                                        )
                                    val match = searchState.results[newIndex]
                                    scrollToMatchIfNeeded(match)
                                    LogUtil.d("TerminalScreen", "Search prev: match row=${match.lineIndex}")
                                    searchState = searchState.copy(currentIndex = newIndex)
                                }
                            },
                            onNext = {
                                if (searchState.hasResults) {
                                    val newIndex =
                                        SearchResult.nextIndex(searchState.currentIndex, searchState.resultCount)
                                    val match = searchState.results[newIndex]
                                    scrollToMatchIfNeeded(match)
                                    LogUtil.d("TerminalScreen", "Search next: match row=${match.lineIndex}")
                                    searchState = searchState.copy(currentIndex = newIndex)
                                }
                            },
                            onClose = {
                                showTextSearch = false
                                searchState = SearchState()
                                searchDebouncer.cancel()
                                searchJob?.cancel()
                                surfaceRef.value?.searchActive = false
                                surfaceRef.value?.clearSearchHighlights()
                            },
                            caseSensitive = searchState.caseSensitive,
                            onCaseSensitiveToggle = { searchState = searchState.copy(caseSensitive = it) },
                            autoCaseSensitive =
                            !searchState.caseSensitive && searchState.query.any { it.isUpperCase() },
                            modifier = Modifier.testTag("TextSearchBar"),
                        )
                    } else {
                        ModifierBar(
                            modifier = Modifier.testTag("ModifierBar"),
                            onKeyClick = { data ->
                                viewModel.writeToPty(data.toByteArray())
                            },
                            onKeyBytesClick = { bytes ->
                                viewModel.writeToPty(bytes)
                            },
                            onConsumeModifiers = {
                                viewModel.consumeOneShotModifiers()
                            },
                            onDrawerClick = {
                                scope.launch { drawerState.open() }
                            },
                            onScrollClick = {
                                viewModel.toggleScrollMode()
                            },
                            scrollActive = state.scrollActive,
                            ctrlState = state.ctrlState,
                            altState = state.altState,
                            onToggleCtrl = {
                                viewModel.cycleCtrlState()
                            },
                            onToggleAlt = {
                                viewModel.cycleAltState()
                            },
                            onLockCtrl = {
                                viewModel.lockCtrlState()
                            },
                            onLockAlt = {
                                viewModel.lockAltState()
                            },
                            textColor = resolvedTerminalTheme.foreground,
                            backgroundColor = resolvedTerminalTheme.background,

                            isAppCursorMode = { viewModel.runtime.bridge()?.isAppCursorMode() == true },
                        )
                    }
                }
            } // 关闭内容盒——终端 Surface 与键栏的位移同源同帧（唯一 ime 状态），
            // 但平移量按内容下沿裁剪，二者不同值是设计而非双位移源
        }
    }
}

/**
 * IME insets 叶节点观察器：键盘动画期间 insets 逐帧变化只重组本节点——
 * 读取发生在 composition，写入 [onChanged] 的状态后，终端区/修饰键栏位移经布局期
 * offset lambda 应用，主组合（位移容器/键栏/搜索层）不随之逐帧重组。
 *
 * 后备扣除：`WindowInsets.ime` 在手势导航下包含底部系统导航条高度，
 * `navigationBarsPadding` 已在根 Box 消费同一高度。不扣除会导致位移恒大一个
 * 导航条高度（约 3 行）：内容较少时终端被顶起约 3 行，内容较多时底部约 3 行被键盘遮挡。
 */
@Composable
private fun WindowImeBottomPx(onChanged: (Int) -> Unit) {
    val density = LocalDensity.current
    val imeBottom = WindowInsets.ime.getBottom(density)
    val navigationBottom = WindowInsets.navigationBars.getBottom(density)
    SideEffect { onChanged(max(imeBottom - navigationBottom, 0)) }
}
