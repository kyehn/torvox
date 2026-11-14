package terminal.emulator.ui

import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import terminal.emulator.TerminalViewModel
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.computeContentBottomPx
import terminal.emulator.runtime.computeImeSurfaceShift

/**
 * 拥有输入法接入：平台 insets 派发、视口随键盘平移与备用屏网格重排。
 *
 * 遮挡高度的唯一来源是平台 insets 派发（见 [installImeInsetListener]）；本类同时
 * 持有由它派生的平移量与网格重排防抖，视图只转发事件与读取结果。
 */
internal class ImeShiftController(private val surface: TerminalSurface) {
    /**
     * 订阅决定平移量与网格的输入：内容下沿（按内容裁剪平移量）、单元格度量
     * （字号与捏合缩放改行高）、备用屏状态（平移恒 0 且需要一次网格重排）。
     * 前两者由渲染线程逐帧发布，故平移量随内容增长即时跟进。
     *
     * 必须在每次 attach 时重建（见 [onAttachedToWindow]）：detach 会停掉它们，
     * 而视图复用不重走 [attachViewModel]。
     */
    internal fun observeInputs(host: TerminalViewModel) {
        imeShiftJob?.cancel()
        imeShiftJob =
            host.viewModelScope.launch {
                // 三个输入各自足以改变平移量或网格：内容下沿（按内容裁剪）、单元格
                // 行列尺寸（字号/捏合；行高变了但行号没变时内容下沿不会发射）、
                // 备用屏（位移恒 0 且需要一次网格重排）。
                launch { host.runtime.lastContentRowFlow.collect { applyImeShift() } }
                launch { host.runtime.cellMetricsFlow.collect { applyImeShift() } }
                launch {
                    host.runtime.altScreenActiveFlow.collect {
                        applyImeShift()
                        scheduleImeGridResize()
                    }
                }
            }
    }

    /**
     * 输入法遮挡高度（px，已扣除系统导航条），由平台 insets 派发维护
     * （[installImeInsetListener]）。本视图用它算平移量（[applyImeShift]）；
     * 网格高度不读这个字段，而是读运行期的
     * [terminal.emulator.runtime.TerminalRuntime.imeGridReserve]——
     * 那是网格扣减的唯一来源，与本字段同源但不可独立修改。
     *
     * 备用屏应用按整屏行数布局（helix/vim/less 都进备用屏并占满视口），键盘遮挡
     * 的下半屏永远不可见：状态行消失、光标可能落在被遮住的几行里，而应用收不到
     * SIGWINCH 也不会重排——即 DESIGN「输入法弹出时终端（包括 helix 等 tui 应用）
     * 正确匹配窗口大小」不成立。扣除遮挡高度即让网格缩到可见高度，触发一次
     * resize/SIGWINCH，全屏 TUI 随之按新窗口重绘；网格顶对齐渲染，末行紧贴键栏
     * 顶边，与 Termux `adjustResize` 的观感一致。
     */
    internal var imeInsetPx: Int = 0
    private var pendingImeGridResize: Runnable? = null
    private var imeShiftJob: Job? = null

    /** 输入法可见性：键盘的显示/隐藏 FLIP 才关闭选区手柄与上下文菜单。 */
    private var lastImeVisible: Boolean = false

    /** 已应用的本视图平移量，避免重复赋值触发无谓的重绘失效。 */
    internal var appliedImeShiftPx: Int = 0

    /**
     * 位移显出量（px）：在**滚动行程尽头之外**按手指位移 1:1 回收位移，
     * 把被位移裁到屏幕上边界外的网格顶部重新放回屏幕。
     *
     * 为什么需要它：位移只按键盘遮挡与内容下沿计算，与滚动位置无关，于是网格
     * 最上面的「位移」像素常驻屏幕外，而上滑行程以回滚区顶端为终点——「到顶部」
     * 在像素上不可达（STATUS「输入法弹出时上滑不能到顶部」）。显出量由
     * [applyScrollDistance] 消费回滚区顶端之外的越界位移得到，回到边界内时先按
     * 同一比例回收（见该函数），两个方向都是 1:1，不出现 2 倍行程。
     */
    internal var imeShiftRevealPx: Float = 0f

    /** [imeShiftRevealPx] 的容量上限：当前完整位移（无键盘、备用屏恒 0）。 */
    internal var imeShiftCapacityPx: Int = 0

    /**
     * 订阅平台 insets 派发，维护 [imeInsetPx]。
     *
     * 为什么是派发而非轮询或组合读取：既有实现读 Compose 的 `WindowInsets.ime`
     * 叶节点并与轮询的 `rootView.rootWindowInsets` 取最大值——两者的**取值**都是
     * 正确的（`ImePopupPixelInstrumentedTest` 就靠后者读到非零高度），失效的是
     * **生效时机**：那条链路的终点是 insets 遍历内的组合状态写入，而这样的写入
     * 不保证被观察到（实测每 300ms 写一次组合状态，20 次才换来一次重组），
     * 位移因此从未发生（`ImePopupPixelInstrumentedTest` 三个用例即以此判红：
     * 位移=0）。派发是平台自己的分发路径，已挂载视图必然收到，且回调直接改视图
     * 属性时在同一拍内生效。
     */
    internal fun installImeInsetListener() {
        ViewCompat.setOnApplyWindowInsetsListener(surface) { view, insets ->
            val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val navigationBottom =
                insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            val reserved = maxOf(imeBottom - navigationBottom, 0)
            if (reserved != imeInsetPx) {
                imeInsetPx = reserved
                LogUtil.debug(TerminalSurface.TAG, "setImeInsetPx: $reserved")
                applyImeShift()
                // 键栏（Compose 覆盖层）也按此高度上移。它只能走组合，故经运行期流发布：
                // 直接回调会在 insets 遍历内写组合状态，那次写入不保证被观察到。
                // 同一次发布也更新网格扣减量（三条网格路径共用，见 imeGridReserve）。
                surface.viewModel?.runtime?.publishImeInsetPx(reserved, surface)
                // 备用屏按可见高度重排网格。与平移不同，它必须等输入法高度稳定
                // （见 [scheduleImeGridResize]），否则逐帧 SIGWINCH 会让全屏 TUI 反复重排。
                scheduleImeGridResize()
            }
            // 键盘的显示/隐藏 FLIP 才关闭选区手柄与上下文菜单——显示时定位的弹窗
            // 绝不会相对滚动而陈旧。以 FLIP（而非每像素变化）为闸门很重要：显示/隐藏
            // 动画每帧都发出 insets，逐帧清除会抹掉动画期间做出的选择。
            //
            // 必须在**本监听器**里做而不能靠 `onApplyWindowInsets`：框架在视图装了就
            // `OnApplyWindowInsetsListener` 时只调它，不再调用 `onApplyWindowInsets`
            // （ViewCompat 的 wrapper 即如此实现），放在重写方法里等于静默失效。
            val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            if (imeVisible != lastImeVisible) {
                lastImeVisible = imeVisible
                surface.viewModel?.clearSelection()
                surface.selectionHandles.hideSelectionHandles()
                surface.hideSelectionMenu("imeVisibilityChanged")
            }
            insets
        }
    }

    /**
     * 把输入法跟随位移直接施加到本视图的 `surface.translationY`。
     *
     * 为什么不平移 Compose 容器（历史做法）：位移要走「组合重组 → 重新测量 →
     * 重新布局」才生效，而在主线程被渲染阻塞时会滞后十几秒（实测见
     * `TerminalRuntime.imeInsetFlow`），期间键盘已经弹出而内容一动不动
     * ——`ImePopupPixelInstrumentedTest` 三个用例即以此判红（位移=0）。
     * `surface.translationY` 是视图自身的属性，在 insets 回调里同步生效，不依赖任何
     * 组合往返。
     *
     * 位移量仍是 [computeImeSurfaceShift] 的判定结果（按内容下沿裁剪、备用屏恒 0），
     * 口径与单测一致。六个触发点各自覆盖它的一个输入：输入法遮挡（insets 派发）、
     * 内容下沿、单元格度量、备用屏状态（后三者由 [observeImeShiftInputs] 订阅）、
     * 视口尺寸（[onSizeChanged]，旋转时 Activity 不重建）、滚动位置
     * （[applyScrollOffset]，见 [imeShiftRevealPx]）。
     */
    internal fun applyImeShift() {
        val runtime = surface.viewModel?.runtime ?: return
        if (surface.height <= 0) return
        val cellHeight = runtime.cellHeight
        val capacity =
            computeImeSurfaceShift(
                contentBottomPx = computeContentBottomPx(runtime.lastContentRowFlow.value, cellHeight),
                // 光标行单独算：它常落在最后一个有字行的下一行（提示符整行占满时），
                // 只报内容末行会让位移差一行，光标留在键盘后面且用户看不到任何线索。
                // 取不到光标（负值）时传 -1，此时该参数为 0，退化为只看内容末行。
                cursorBottomPx =
                computeContentBottomPx(
                    runtime.bridge()?.renderedCursorPacked()
                        ?.takeIf { it >= 0 }
                        ?.let { (it shr 32).toInt() }
                        ?: -1,
                    cellHeight,
                ),
                surfaceHeightPx = surface.height,
                modifierBarHeightPx = runtime.modifierBarHeightPx,
                imeBottomPx = imeInsetPx,
                isAltScreenActive = runtime.altScreenActiveFlow.value,
            )
        // 显出量不得越过「回到底部还需上滑的行程」：偏移到 0（实时视图）必须归零，
        // 否则提示符被键盘吞掉（TESTING.md 第 37 条）。到回滚区顶端时该上界等于整段
        // 回滚，恒大于容量，不参与限制。
        imeShiftRevealPx =
            imeShiftRevealPx.coerceIn(0f, minOf(capacity.toFloat(), surface.scrollOffset * cellHeight))
        imeShiftCapacityPx = capacity
        val shift = capacity - imeShiftRevealPx.toInt()
        if (shift == appliedImeShiftPx) return
        appliedImeShiftPx = shift
        surface.translationY = -shift.toFloat()
    }

    /**
     * 输入法遮挡或备用屏状态变化后按可见高度重排网格。Surface 尺寸本身不变
     * （窗口是 `adjustNothing`），故只重算网格、不重配交换链。
     *
     * 防抖到稳定高度：键盘动画期间平台逐帧派发 insets，逐帧 resize 会连续发
     * SIGWINCH 让全屏 TUI 反复重排（掉帧、撕裂、耗电）。
     */
    private fun scheduleImeGridResize() {
        if (surface.surfaceWidthPixels <= 0 || surface.surfaceHeightPixels <= 0) return
        val runtime = surface.viewModel?.runtime ?: return
        // 切后台期间输入法显隐会异步触发 insets 回调：此时不应排队网格重排或持有
        // 备用屏的渲染暂停凭证，否则防抖可能在后台/恢复窗口期执行并干扰前后台切换。
        // 返回前台后 insets 会按真实状态再次派发，自然触发需要的重排。
        if (runtime.isBackgroundPaused) {
            cancelGridResize()
            releaseGridResizePause()
            return
        }
        // 主屏 MUST NOT 暂停渲染：那里的遮挡计为 0，`applyGridResize` 算出的行数与
        // 当前相同，是个 no-op——而键盘动画期间平台逐帧派发 insets，每次都重新起算
        // 防抖，暂停会一直挂到动画结束，终端停止上帧（TESTING.md 要求弹/收键盘不卡顿
        // 无闪烁）。但重排本身 MUST 仍然发生：离开备用屏时（键盘可能仍展开）网格要
        // 按整屏高度复原，此时 `imeGridReserve()` 自然为 0。若在这里一并跳过，
        // PTY 会停留在被输入法缩小后的行数，且此后无任何自愈触发点。
        // 先领新的、再归还旧的（理由见 applySurfaceResize）：两者都持有时持有者数
        // 走 1 → 2 → 1，全程不落到 0，不发出多余的恢复/暂停回调。防抖窗内备用屏状态
        // 可能翻转，旧的持一次而新的不持——那种情况下持有者数 1 → 1 → 0，恰好发一次
        // 恢复，是正确的。
        //
        // 备用屏下新旧行数交替渲染会被用户看见，故在防抖期间暂停渲染。
        val token = if (runtime.altScreenActiveFlow.value) runtime.acquireRenderPause() else null
        pendingImeGridResize?.let { surface.removeCallbacks(it) }
        pendingImeGridResize = null
        pendingImeGridResizeToken?.let { runtime.releaseRenderPause(it) }
        pendingImeGridResizeToken = token
        pendingImeGridResize =
            Runnable {
                pendingImeGridResize = null
                // 保险：防抖触发时若已切后台，丢弃本次重排并释放凭证，避免恢复窗口期干扰。
                if (runtime.isBackgroundPaused) {
                    token?.let { runtime.releaseRenderPause(it) }
                    if (pendingImeGridResizeToken == token) pendingImeGridResizeToken = null
                    return@Runnable
                }
                surface.resizeManager.applyGridResize(surface.surfaceWidthPixels, surface.surfaceHeightPixels)
                // 凭证已陈旧时（视图 detach 后的 `mRunQueue` 延迟派发）ledger 会把它当
                // 空操作，故这里只清仍属于本次的那一个。
                token?.let { runtime.releaseRenderPause(it) }
                if (pendingImeGridResizeToken == token) pendingImeGridResizeToken = null
            }.also { surface.postDelayed(it, TerminalSurface.IME_RESIZE_DEBOUNCE_MS) }
    }

    /** 在途 [pendingImeGridResize] 的暂停凭证（主屏不暂停，故可为空）。 */
    private var pendingImeGridResizeToken: Long? = null

    /** 视图拆除时归零本类持有的全部输入法状态。 */
    internal fun reset() {
        appliedImeShiftPx = 0
        imeShiftCapacityPx = 0
        imeShiftRevealPx = 0f
        imeInsetPx = 0
        cancelGridResize()
        imeShiftJob?.cancel()
        imeShiftJob = null
    }

    /** 取消在途的输入法网格重排防抖。 */
    internal fun cancelGridResize() {
        pendingImeGridResize?.let { surface.removeCallbacks(it) }
        pendingImeGridResize = null
    }

    /** 归还输入法网格重排防抖持有的渲染暂停。 */
    internal fun releaseGridResizePause() {
        pendingImeGridResizeToken?.let { surface.viewModel?.runtime?.releaseRenderPause(it) }
        pendingImeGridResizeToken = null
    }
}
