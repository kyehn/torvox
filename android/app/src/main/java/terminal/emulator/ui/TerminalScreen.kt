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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import terminal.emulator.TerminalViewModel
import terminal.emulator.runtime.LogUtil
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName
import terminal.emulator.util.TerminalDispatchers
import kotlin.math.max
import kotlin.math.min

// toggleKeyboard lambda 在抽屉完全关闭时已跳过延迟路径；
// 抽屉打开时，50ms 足以让遮罩轻击在输入法与关闭动画竞争前生效。
// 原先的 250ms 会被感知为输入延迟。
private const val IME_TOGGLE_DELAY_MS = 50L

/**
 * 搜索查询串长度上限：向原生取真实值（原生为唯一真源，不在此处自带副本）。
 * 超长查询会被原生直接判为无命中，故 UI 必须先截断。
 */
private val searchQueryMaxLength: Int by lazy {
    terminal.emulator.bridge.NativeBridge.searchQueryMaxChars()
}

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
    // 未知名已在 TerminalRuntime.clearUnknownThemeNames 清除对应键；此处取不到
    // 说明该键刚被写入但尚未生效，用默认主题渲染本帧，不抛。
    val resolvedTerminalTheme =
        BuiltInThemes.byNameOrNull(
            resolveTerminalThemeName(
                mode = viewModelThemeMode,
                fixedName = viewModelThemeName,
                dayName = viewModelDayThemeName,
                nightName = viewModelNightThemeName,
                isDark = isSettingsDark,
            ),
        ) ?: BuiltInThemes.draculaPlus
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
    SystemBarsFollowTerminalBackground(view, terminalBackground)
    val surfaceRef = remember { mutableStateOf<TerminalSurface?>(null) }
    // 切换软键盘（termux 的 KEYBOARD 键）：供会话抽屉的键盘按钮使用
    // （修饰键栏布局不可配置，故没有 KEYBOARD 附加键，见 PROHIBITED 的布局编辑器禁令）。
    // 可见性在轻点时从已挂载的 window insets 同步读取：这里是用户手势，
    // 允许同步读一次平台状态；而 IME 高度那条链路不行——它必须靠持续派发，
    // 因为回调里写组合状态不保证被观察到（见 imeInsetFlow 的说明）。
    val toggleKeyboard: () -> Unit = {
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
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) {
                // 组合顶层的 LocalView.current 是 AndroidComposeView 而非 TerminalSurface；
                // 故使用从 AndroidView 工厂捕获的 surfaceRef。
                surfaceRef.value?.finishComposing()
                // 取消在途的 surface 重建重试：它会 setRenderPaused(false) +
                // resumeRendering()，跨过本次暂停就会在已被系统回收的 BufferQueue
                // 上继续出帧（ERROR_SURFACE_LOST_KHR → 返回后永久黑屏）。
                surfaceRef.value?.cancelSurfaceRecreate()
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
                surfaceRef.value?.hideSelectionMenu("backHandler")
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
                // 原生侧要逐回滚行拼串 + 跑正则（上限 5 万匹配），在 5 万行回滚上以秒计。
                // 阻塞 JNI 必须离开主线程：否则输入法与按键一起卡住。
                val matches =
                    withContext(TerminalDispatchers.inputOutput) {
                        bridge.searchAllInScrollback(query, effectiveCaseSensitive)
                    } ?: run {
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
                    // 必须一并取消待执行的防抖搜索：它带着**切换前**的大小写敏感度，
                    // 在 150ms 后才启动并覆盖 searchState，于是界面显示大小写不敏感的
                    // 开关状态却配着区分大小写的结果集。
                    searchDebouncer.cancel()
                    searchJob?.cancel()
                    searchJob = scope.launch { performSearch() }
                }
            }

            // IME 跟随：位移由两处各自承担，二者读同一个 insets 来源（`imeInsetFlow`）。
            // 终端 Surface 在 insets 派发回调里直接改自身 `translationY`；键栏是
            // 组合覆盖层，只能读流。不再平移 `TerminalContent` 容器：那要经
            // 「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer 帧回调里跑，
            // 主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时滞后十几秒（实测每
            // 300ms 写一次组合状态，20 次才换来一次重组），键盘已弹出而内容不动
            // ——`ImePopupPixelInstrumentedTest` 三个用例即以此判红。
            // 两者同源，取值必然一致；生效时刻不同（键栏要等一次重组），键盘动画
            // 期间会看到终端先上移、键栏随后跟上。
            //
            // 高度只有一个来源：终端 Surface 上的平台 insets 派发回调，经运行期
            // `imeInsetFlow` 汇入组合（`TerminalSurface.installImeInsetListener`
            // → `TerminalRuntime.publishImeInsetPx`）。
            // 此前读 Compose `WindowInsets.ime` 叶节点与轮询 `rootWindowInsets` 取
            // 最大值——两者**取值**都对，失效的是生效时机：那条链路的终点是 insets
            // 遍历内的组合状态写入，不保证被观察到，故位移从未发生。
            val imeInsetPx by viewModel.runtime.imeInsetFlow.collectAsStateWithLifecycle()
            val runtimeForContent = viewModel.runtime
            // 换视图的触发值必须在此处（组合体自身）读取：读在 `Box` 的内容 lambda 里时，
            // 该 lambda 的捕获未变会被 Compose 跳过，`key(...)` 也就不会被重新求值，
            // 失效信号再真实也换不掉视图（实测信号已送达却无重组）。读在组合体里则
            // 本组的重启作用域直接记录该读，信号一变即重组，并把新值作为捕获传给
            // lambda，lambda 随之重跑。
            val surfaceKey = runtimeForContent.surfaceRecreateSignal.intValue
            // 原生 surface 判死（其原生窗口的 BufferQueue 被遗弃）时换掉整个
            // `SurfaceView`：`key` 变更使旧视图被拆除（`surfaceDestroyed` 释放
            // wgpu surface）、新视图重新 `surfaceCreated` 交付**新的**原生窗口，
            // 原生随之走重建慢路径。对同一窗口反复 detach/attach 唤不活被遗弃的
            // BufferQueue（实测此后每帧 `begin_frame failed`、终端永久黑屏）。

            // 本容器是终端 Surface 与键栏的共同父级，但**不**承担输入法位移：
            // 平移量按内容下沿裁剪（网格自顶端锚定渲染，键盘遮住的是末尾行，按整块
            // 键盘高度上移会把稀疏会话的提示符推出屏幕上边界），而网格尺寸在主屏全程
            // 不变（改网格会带来重排闪烁与底部行丢失）。
            //
            // 终端 Surface 在 insets 派发回调里直接改自身 `translationY`（见其
            // `applyImeShift`）：走组合要经「重组 → 重新测量 → 重新布局」，而重组只在
            // Choreographer 帧回调里跑，主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染
            // 线程时滞后可达十几秒（实测每 300ms 写一次组合状态，20 次才换来一次重组），
            // 期间键盘已弹出而内容纹丝不动。键栏是组合覆盖层，读同一个 `imeInsetFlow`
            // 上移——两者同源，不会出现一个跟上一个不跟的差拍。
            Box(
                modifier =
                Modifier.fillMaxSize()
                    .testTag("TerminalContent"),
            ) {
                // 终端 Surface 占满整块高度：键栏覆盖其底部，而网格已按同一口径预留
                // 键栏高度（见 TerminalSurface.ResizeManager），故 rows/cols 不受键栏位移影响。
                key(surfaceKey) {
                    Box(modifier = Modifier.fillMaxSize()) {
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
                                        onZoomChanged = { sizeSp ->
                                            // ⑥ 双指缩放终结：持久化稳定尺寸并执行完整应用
                                            // （单次网格重排）。
                                            // 尺寸已由 zoomFontSize 钳到设置调节条同范围，此处不再重钳。
                                            viewModel.setFontSize(sizeSp)
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

                            // 以整个选区状态与承载它的 Surface 为 key 的单一 effect：
                            // key 变化时先取消正在运行的 effect，
                            // 故未改变锚点单元格就结束的拖动不会把手柄留在隐藏状态
                            // （此前拆分的显示/隐藏 effect 会在软件渲染器上乱序执行，
                            // 在显示之后又隐藏）。
                            // surfaceRef 必须在 key 里：AndroidView 重建 SurfaceView 时
                            // 旧的已脱离窗口，不重启 effect 就再无手柄（选区仍在）。
                            LaunchedEffect(
                                selectionActive,
                                selection.dragging,
                                loRow,
                                loCol,
                                hiRow,
                                hiCol,
                                themeAccentArgb,
                                surfaceRef.value,
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
                                // surfaceRef 必须在 key 里：PopupWindow 是由**那一个**
                                // Surface 承载的独立系统窗口，AndroidView 重建 SurfaceView 后
                                // 旧 Surface 已脱离窗口，`showSelectionMenu` 会走「未附着」分支
                                // 静默返回；key 不含它则 effect 不重启，菜单就此永久缺席。
                                LaunchedEffect(selection.pasteOnly, selection.menuDismissed, menuSurface) {
                                    if (selection.menuDismissed) {
                                        menuSurface.hideSelectionMenu("selectionFlowDismissed")
                                    } else {
                                        menuSurface.showSelectionMenu(selection.pasteOnly)
                                    }
                                }
                            } else {
                                LaunchedEffect(Unit) { menuSurface.hideSelectionMenu("noMenuSurface") }
                            }
                        } else {
                            LaunchedEffect(selectionActive) {
                                menuSurface?.hideSelectionMenu("selectionInactive")
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
                }

                // 键栏覆盖在 Surface 底部（网格已预留其高度），按整块键盘高度上移——
                // 恒位于输入法上方而不被遮挡。它与终端 Surface 取同一个 ime 高度
                // （`imeInsetFlow`），但生效时刻不同：本偏移要等一次重组，
                // Surface 的 `translationY` 在 insets 派发的同一拍内生效。
                Box(
                    modifier =
                    Modifier.align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .offset {
                            IntOffset(0, -imeInsetPx)
                        }
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
                                searchState = searchState.copy(query = newQuery.take(searchQueryMaxLength))
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
                                val sessionId = viewModel.runtime.inputTargetSessionId
                                viewModel.writeToPty(sessionId, data.toByteArray(Charsets.UTF_8))
                            },
                            onKeyBytesClick = { bytes ->
                                viewModel.writeToPty(viewModel.runtime.inputTargetSessionId, bytes)
                            },
                            onConsumeModifiers = {
                                viewModel.consumeOneShotModifiers()
                            },
                            onDrawerClick = {
                                scope.launch { drawerState.open() }
                            },
                            onPasteClick = {
                                viewModel.pasteFromClipboard()
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
            } // 关闭内容盒——终端 Surface 与键栏读同一个 ime 高度，但平移量按内容
            // 下沿各自裁剪（键栏恒为整块键盘高度），故两者位移值不同是设计而非双位移源
        }
    }
}

/**
 * 系统栏图标明暗跟随已解析的终端主题背景。
 *
 * 窗口是边到边的（`MainActivity.onCreate` 已 `WindowCompat.enableEdgeToEdge`），
 * 系统栏自身透明、只决定**图标明暗**，而 `enableEdgeToEdge` 的默认样式跟随
 * **系统**深浅色——与应用内「日间/夜间」开关以及浅色终端主题都无关：
 * 日间 + 浅色终端主题下状态栏图标恒为浅色，在浅背景上不可见
 * （DESIGN：软件主题三种模式；终端配色作用于终端页面与修饰键栏）。
 *
 * 明暗判据取背景自身亮度而非软件主题开关：图标最终画在该背景像素上，
 * 按像素判才不会在浅色终端主题配夜间开关时判反。
 */
@Composable
private fun SystemBarsFollowTerminalBackground(view: android.view.View, background: Color) {
    val window = (view.context as? android.app.Activity)?.window ?: return
    val lightBackground = usesLightSystemBarIcons(background)
    SideEffect {
        // 显式声明图标明暗：`WindowCompat.enableEdgeToEdge(window)` 按**系统**深浅色
        // 决定，且不接受样式参数；`SystemBarStyle` 那套只挂在已弃用的
        // `ComponentActivity.enableEdgeToEdge` 上。要让图标跟随应用内主题与终端配色，
        // 只能直接写 insets controller 的两枚开关。
        androidx.core.view.WindowInsetsControllerCompat(window, view).apply {
            isAppearanceLightStatusBars = lightBackground
            isAppearanceLightNavigationBars = lightBackground
        }
    }
}

/**
 * 系统栏图标是否取深色：背景为浅色时取深色图标（WCAG 相对亮度中点为界）。
 *
 * 按背景像素而非软件主题开关判定：图标最终画在该背景上，浅色终端主题配夜间开关时
 * 按开关判会判反。
 */
internal fun usesLightSystemBarIcons(background: Color): Boolean =
    background.luminance() > LIGHT_BACKGROUND_LUMINANCE_THRESHOLD

/** 背景亮度高于此值即按浅色背景处理（图标取深色）。取 WCAG 相对亮度中点。 */
private const val LIGHT_BACKGROUND_LUMINANCE_THRESHOLD = 0.5f
