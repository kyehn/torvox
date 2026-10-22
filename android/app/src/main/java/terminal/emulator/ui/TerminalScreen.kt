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
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.material3.DrawerState
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
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import terminal.emulator.TerminalViewModel
import terminal.emulator.bridge.SearchMatchRange
import terminal.emulator.runtime.LogUtil
import terminal.emulator.ui.theme.ThemeRepository
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
 * 超长查询在 UI 侧直接判为无命中，不送原生（见 [coerceSearchQuery]）。
 */
private val searchQueryMaxLength: Int by lazy {
    // 原生库缺失时 NativeBridge 的类初始化仍会成功（它只记日志），
    // 但 external 方法的链接会抛 UnsatisfiedLinkError——那是 Error 不是 Exception，
    // 会从 onQueryChange 回调一路逃到全局处理器而崩掉进程。故显式设防。
    val bridge = terminal.emulator.bridge.NativeBridge
    if (!bridge.isNativeLoaded()) {
        0
    } else {
        try {
            bridge.searchQueryMaxChars()
        } catch (error: UnsatisfiedLinkError) {
            LogUtil.e("TerminalScreen", "searchQueryMaxChars is unavailable: ${error.message}")
            0
        }
    }
}

/** 终端内文本搜索状态，取代原先 6 个独立的 remember 变量。 */
private data class SearchState(
    val query: String = "",
    val resultCount: Int = 0,
    val currentIndex: Int = 0,
    val selectedMatch: SearchMatchRange? = null,
    val highlightsActive: Boolean = false,
) {
    val hasResults: Boolean
        get() = resultCount > 0
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
    // 开孔是否已有缓冲区：决定要不要在窗口层涂底色（见 TerminalRuntime.surfaceBufferPresent）。
    val surfaceBufferPresent by viewModel.runtime.surfaceBufferPresent.collectAsStateWithLifecycle()
    val isSettingsDark =
        resolveAppDarkMode(settings.appThemeMode, androidx.compose.foundation.isSystemInDarkTheme())
    // 未知名已在 TerminalRuntime.clearUnknownThemeNames 清除对应键；此处取不到
    // 说明该键刚被写入但尚未生效，用默认主题渲染本帧，不抛。
    val resolvedTerminalTheme =
        ThemeRepository.byNameOrNull(
            resolveTerminalThemeName(
                mode = viewModelThemeMode,
                fixedName = viewModelThemeName,
                dayName = viewModelDayThemeName,
                nightName = viewModelNightThemeName,
                isDark = isSettingsDark,
            ),
        ) ?: ThemeRepository.byName(ThemeRepository.defaultDarkName())
    val terminalBackground = resolvedTerminalTheme.background
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    // 只在事件回调里读写，从不在组合期读，故不用快照状态——那会让每次赋值多触发
    // 一次无效失效（写入还发生在 Handler 回调里，本就不在快照范围内）。
    val searchJobHolder = remember { java.util.concurrent.atomic.AtomicReference<kotlinx.coroutines.Job?>(null) }
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
    // 覆盖层（设置页）打开时终端页不得再写系统栏明暗：设置页与终端页同处一棵
    // 组合树，各自都写的话谁最后重组谁赢——被遮住的那一页也会把图标改成另一套，
    // 表现为「切回终端瞬间图标闪一下」。覆盖层自己按自己的底色写。
    if (!isOverlayVisible) {
        SystemBarsFollowBackground(view, terminalBackground)
    }
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
                // 切后台：领一次渲染暂停并停止渲染线程。切应用不会调用
                // surfaceDestroyed（Surface 被保留），不做此步线程会继续在已被
                // 系统回收的 BufferQueue 上取帧 → ERROR_SURFACE_LOST_KHR
                // → 返回后永久黑屏（模拟器已验证）。直写全局布尔则会被随后的前台
                // 恢复（或关设置页）把别的来源仍在等的暂停一并清掉，故一律经账本。
                //
                // 顺序 MUST 是「先领新的、再取消旧的」：反之会让持有者数经过 0，
                // 在一个即将被系统回收的窗口上多出一对暂停/恢复。
                viewModel.runtime.pauseRenderingForBackground()
                // 在途的两个尺寸防抖同样必须取消：后台时 SurfaceView 保留窗口，
                // 它们却仍会到期执行并恢复渲染，把刚装上的后台暂停撤掉。
                surfaceRef.value?.cancelResizeDebounces()
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
                    // 不在这里无条件 release/attach：Surface 被保留时交换链通常仍然可用，
                    // 重建会先丢掉上一帧、再等新交换链的首帧，正是切回时看到的一闪。
                    // 真的被系统回收时，渲染线程会在取帧失败后随帧上报
                    // `surfaceInvalidated`，由 `maybeRequestSurfaceRecreate` 换成新窗口；
                    // `surfaceDestroyed` 路径则由 `surfaceCreated` 重新绑定。
                    viewModel.runtime.clearLifecycleRenderPause()
                    viewModel.runtime.resumeRendering()
                    // 即使没有新输出也立即出一帧：交换链若已失效，这一帧就是探针。
                    viewModel.runtime.forceRender()
                } else {
                    surface?.postDelayedUnpause(200L)
                }
                view.requestFocus()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            // 组合在此期间被销毁而 ON_RESUME 未走到（Surface 始终未就绪）时，
            // 切后台那次持有就没人还了——账本里留着它，此后所有领用都不再
            // 发出暂停。凭证在运行期（不随组合走），故此处显式归还。
            viewModel.runtime.resumeRenderingFromBackground()
        }
    }

    BackHandler(enabled = drawerState.isOpen) {
        scope.launch { drawerState.close() }
    }

    // 抽屉面板要避开状态栏：见 DrawerSheet。
    val statusBarTop = with(LocalDensity.current) { WindowInsets.statusBars.getTop(this).toDp() }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            DrawerSheet(statusBarTop = statusBarTop) {
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

            suspend fun performSearch() {
                val query = searchState.query
                if (query.isEmpty()) {
                    searchState = searchState.copy(resultCount = 0, selectedMatch = null)
                    return
                }
                surfaceRef.value?.finishComposing()
                val bridge =
                    viewModel.runtime.bridge()
                        ?: run {
                            searchState = searchState.copy(resultCount = 0, selectedMatch = null)
                            return
                        }
                // 原生侧要跑上游搜索器（上限 50k 命中），在 5 万行回滚上以秒计。
                // 阻塞 JNI 必须离开主线程：否则输入法与按键一起卡住。
                val total =
                    withContext(TerminalDispatchers.inputOutput) {
                        bridge.searchSetNeedle(query)
                    }
                // 换 needle 即重启搜索，上一次的选中态作废（上游不保留旧命中）。
                searchState = searchState.copy(resultCount = total, currentIndex = 0, selectedMatch = null)
            }

            /**
             * 把选中命中居中：上游定位策略只保证命中可见，命中会贴视口上下缘，
             * 上下文为零。居中由视图侧执行，命中行上下各留半屏可读内容。
             */
            fun centerSelectedRow(row: Int) {
                val surface = surfaceRef.value ?: return
                val centeredRow = (row - surface.getRows() / 2).coerceAtLeast(0)
                surface.scrollToRow(centeredRow)
            }

            /** 上一个/下一个：导航由上游完成（顺带滚动视口），此处只消费选中态。 */
            suspend fun selectSearchMatch(forward: Boolean) {
                val bridge = viewModel.runtime.bridge() ?: return
                val selection =
                    withContext(TerminalDispatchers.inputOutput) {
                        bridge.searchSelect(forward)
                    } ?: run {
                        searchState = searchState.copy(selectedMatch = null)
                        return
                    }
                searchState =
                    searchState.copy(
                        currentIndex = selection.index,
                        resultCount = selection.total,
                        selectedMatch = selection.matched,
                    )
                centerSelectedRow(selection.matched.row)
            }

            // IME 跟随：位移由两处各自承担，二者读同一个 insets 来源（`imeInsetFlow`）。
            // 终端 Surface 在 insets 派发回调里直接改自身 `translationY`；键栏是
            // 组合覆盖层，只能读流。不再平移 `TerminalContent` 容器：那要经
            // 「重组 → 重新测量 → 重新布局」，在主线程被渲染阻塞时会滞后十几秒
            // （实测见 `TerminalRuntime.imeInsetFlow`），键盘已弹出而内容不动。
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
            // `applyImeShift`）：走组合会滞后（依据见 `TerminalRuntime.imeInsetFlow`）。
            // 键栏是组合覆盖层，读同一个 `imeInsetFlow` 上移——取值同源故必然一致，
            // 但生效时刻不同：Surface 同一拍内生效，键栏要等一次重组。
            Box(
                modifier =
                Modifier.fillMaxSize()
                    .testTag("TerminalContent"),
            ) {
                // 终端 Surface 占满整块高度：键栏覆盖其底部，而网格已按同一口径预留
                // 键栏高度（见 TerminalSurface.ResizeManager），故 rows/cols 不受键栏位移影响。
                key(surfaceKey) {
                    Box(
                        modifier =
                        Modifier.fillMaxSize()
                            // SurfaceView 的原生 surface 在窗口**之下**：窗口这块是透明
                            // 开孔，缓冲区缺失时（冷启动首帧前、切后台回来重建窗口时）
                            // 透出后方桌面即「切应用返回一闪」。开孔遮不住（窗口像素盖在
                            // surface 之上），故只在「还没有缓冲区」这窗口内涂终端底色，
                            // 一有帧提交就撤（见 TerminalRuntime.surfaceBufferPresent）。
                            .then(
                                if (surfaceBufferPresent) {
                                    Modifier
                                } else {
                                    Modifier.background(terminalBackground)
                                },
                            ),
                    ) {
                        // 工厂在**组合期**被调用（ComposeNode 创建阶段），而
                        // `surfaceRef` 在本组合内被读（LaunchedEffect 的 key）——
                        // 组合期写快照状态会让整个 TerminalScreen 再走一遍组合，
                        // 首遍还按 `surfaceRef == null` 挂上另一条 effect 分支。
                        // 故工厂只把视图放进非快照持有者，由随后的 SideEffect
                        // （组合提交后）写入快照状态。
                        val createdSurface = remember { arrayOfNulls<TerminalSurface>(1) }
                        AndroidView(
                            factory = { context ->
                                terminal.emulator.ui
                                    .TerminalSurface(context)
                                    .apply { setTag("TerminalSurfaceView") }
                                    .also { surface ->
                                        createdSurface[0] = surface
                                        surface.attachViewModel(viewModel)
                                        surface.onScrollChanged = { offset ->
                                            viewportScrollOffset.intValue = offset
                                            viewModel.runtime.setScrollOffset(offset)
                                        }
                                        surface.onScrollingStateChanged = { isScrolling ->
                                            viewModel.runtime.setScrollGestureActive(isScrolling)
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
                        // 组合提交后再发布：组合期写会让本作用域立即再组合一遍。
                        SideEffect { surfaceRef.value = createdSurface[0] }

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
                            searchState.selectedMatch,
                            viewportScrollOffset.intValue,
                        ) {
                            if (showTextSearch && searchState.hasResults) {
                                val surface = surfaceRef.value
                                if (surface != null) {
                                    // 命中区间、选中态、视口折算与高亮色全在原生侧：
                                    // 视图只给滚动偏移，高亮即刻随原生搜索状态同步。
                                    surface.applySearchHighlights(viewportScrollOffset.intValue)
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
                                // 匹配文本长度须受限（`text-search` 查询上限条）：超长直接判无命中，
                                // MUST NOT 截断——截断会让用户搜到另一个串的结果。查询框保留原文。
                                val coerced = coerceSearchQuery(newQuery, searchQueryMaxLength)
                                if (coerced == null) {
                                    searchState =
                                        searchState.copy(query = newQuery, resultCount = 0, selectedMatch = null)
                                    searchJobHolder.get()?.cancel()
                                    searchDebouncer.cancel()
                                } else {
                                    searchState = searchState.copy(query = coerced)
                                    searchJobHolder.get()?.cancel()
                                    // 防抖：连续击键在静默 150ms 后合并为一次 performSearch
                                    // （termlib / ghostty-android 做法）。待执行的搜索由
                                    // debouncer 取消，而非每次击键都新起协程。
                                    searchDebouncer.submit {
                                        searchJobHolder.set(scope.launch { performSearch() })
                                    }
                                }
                            },
                            resultCount = searchState.resultCount,
                            // 只有按过上一个/下一个才有选中命中：未选中时报当前序号是假话。
                            currentResultIndex = searchState.currentIndex.takeIf { searchState.selectedMatch != null },
                            onPrevious = {
                                if (searchState.hasResults) {
                                    scope.launch { selectSearchMatch(forward = false) }
                                }
                            },
                            onNext = {
                                if (searchState.hasResults) {
                                    scope.launch { selectSearchMatch(forward = true) }
                                }
                            },
                            onClose = {
                                showTextSearch = false
                                searchState = SearchState()
                                searchDebouncer.cancel()
                                searchJobHolder.get()?.cancel()
                                surfaceRef.value?.clearSearchHighlights()
                            },
                            modifier = Modifier.testTag("TextSearchBar"),
                        )
                    } else {
                        // 按键回调整体 `remember`：强跳过模式（Compose 编译器 2.x 起
                        // 默认开启）按**实例**比较 lambda，每次重组现造的必然是不同实例，
                        // 该组合永不被跳过，两行按键与分页器整棵子树每次都重跑（此前用
                        // `@NonRestartableComposable` 压掉编译器告警，等于把根因当症状治）。
                        // 构造放在非 @Composable 函数里：compose-lints 的
                        // `ComposeViewModelForwarding` 规则禁止组合体直接引用 ViewModel
                        // 参数并转交下去。
                        val modifierBarActions =
                            remember(viewModel, scope, drawerState) {
                                buildModifierBarActions(viewModel, scope, drawerState)
                            }
                        ModifierBar(
                            modifier = Modifier.testTag("ModifierBar"),
                            onSubmit = modifierBarActions.onSubmit,
                            onKey = modifierBarActions.onKey,
                            onConsumeModifiers = modifierBarActions.onConsumeModifiers,
                            onDrawerClick = modifierBarActions.onDrawerClick,
                            onPasteClick = modifierBarActions.onPasteClick,
                            onScrollClick = modifierBarActions.onScrollClick,
                            scrollActive = state.scrollActive,
                            ctrlState = state.ctrlState,
                            altState = state.altState,
                            onToggleCtrl = modifierBarActions.onToggleCtrl,
                            onToggleAlt = modifierBarActions.onToggleAlt,
                            onLockCtrl = modifierBarActions.onLockCtrl,
                            onLockAlt = modifierBarActions.onLockAlt,
                            textColor = resolvedTerminalTheme.foreground,
                            backgroundColor = resolvedTerminalTheme.background,
                        )
                    }
                }
            } // 关闭内容盒——终端 Surface 与键栏读同一个 ime 高度，但平移量按内容
            // 下沿各自裁剪（键栏恒为整块键盘高度），故两者位移值不同是设计而非双位移源
        }
    }
}

/**
 * 抽屉面板。
 *
 * 面板背景必须避开状态栏：Material3 把 `windowInsetsPadding` 施加在面板的**内容**而非
 * `Surface`（背景）上，面板底色会一路铺到屏幕顶端，把浅色面板盖在状态栏区。状态栏区于是
 * 一半是浅面板（白字时钟不可读）、一半是深色终端，而系统栏图标明暗只有一个全局开关，
 * 无法同时让两侧都清晰。
 *
 * 故面板整体按状态栏高度下移：状态栏区统一露出遮罩下的终端底色，图标继续跟随终端底色。
 */
@Composable
internal fun DrawerSheet(
    statusBarTop: Dp,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    ModalDrawerSheet(
        drawerContainerColor = MaterialTheme.colorScheme.surface,
        // 顶边已由下面的 padding 让开，内容只需处理横向与底部（导航栏）内边距。
        windowInsets = WindowInsets.systemBars.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom),
        modifier = modifier.padding(top = statusBarTop).testTag("DrawerSheet"),
    ) {
        content()
    }
}

/**
 * 构造修饰键栏的回调集合。
 *
 * 刻意放在非 `@Composable` 函数里：compose-lints 的 `ComposeViewModelForwarding`
 * 规则禁止组合体直接引用并转交 ViewModel 参数（[TerminalScreen] 的
 * `viewModel` 是带默认值的参数）。调用点只需
 * `remember(viewModel, scope, drawerState) { buildModifierBarActions(...) }`
 * 一次，即可让 `ModifierBar` 的全部 lambda 参数在重组间保持同一实例。
 *
 * 捕获的都是重组间稳定的值，故按这些键记忆不会读到过期引用。
 */
private fun buildModifierBarActions(
    viewModel: TerminalViewModel,
    scope: CoroutineScope,
    drawerState: DrawerState,
): ModifierBarActions = ModifierBarActions(
    // 文本输入页：输入法文本不经按键编码器，原样直写。
    onSubmit = { data: String ->
        val sessionId = viewModel.runtime.inputTargetSessionId
        viewModel.writeToPty(sessionId, data.toByteArray(Charsets.UTF_8))
    },
    // 按键页：统一走原生键编码器，模式（DECCKM、modifyOtherKeys、kitty 键盘
    // 协议）由终端状态决定，Kotlin 不再自带序列。
    onKey = { keyCode: Int, modifiers: Int ->
        viewModel.runtime.bridge()?.processKeyEvent(keyCode, modifiers, 0, 0, 0) == true
    },
    onDrawerClick = { scope.launch { drawerState.open() } },
    onScrollClick = { viewModel.toggleScrollMode() },
    onToggleCtrl = { viewModel.cycleCtrlState() },
    onToggleAlt = { viewModel.cycleAltState() },
    onLockCtrl = { viewModel.lockCtrlState() },
    onLockAlt = { viewModel.lockAltState() },
    onPasteClick = { viewModel.pasteFromClipboard() },
    onConsumeModifiers = { viewModel.consumeOneShotModifiers() },
)
