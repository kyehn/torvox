package terminal.emulator.ui

import android.view.WindowInsets.Type
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import terminal.emulator.TerminalViewModel
import terminal.emulator.runtime.LogUtil
import kotlin.math.round

/**
 * 内容下沿像素：视口最后一个有内容的行的下沿（视口全空为 0）。
 *
 * 原生行顶为 `row * cellHeight`（浮点，见 `cell_builder.rs` 的 `quad_origin`），
 * 故此处必须先浮点乘后取整。先 `toInt` 再乘会每行丢掉小数并随行数累积
 * （N 行累积误差 = N × 小数部分），位移偏小、末行被键栏吞掉且随内容增多扩大。
 * 取最近整数：浮点乘自带表示误差（如 50×45.6 得 2279.9999），`round` 恰好回到
 * 本意像素边界；`ceil` 会把 2279.9999 进成 2280 后在某些对齐下多移 1px，
 * 整屏文字错开 1px 即产生数千边缘差异像素，违背稀疏会话「无变化」。
 */
internal fun computeContentBottomPx(contentRow: Int, cellHeightPx: Float): Int {
    if (contentRow < 0 || cellHeightPx <= 0f) return 0
    return round((contentRow + 1) * cellHeightPx).toInt()
}

/**
 * 输入法弹出时终端 Surface 的上移像素：只移「键盘遮住且上方放不下」的内容高度。
 *
 * 网格自顶端锚定渲染，键盘遮住的是网格**末尾**行，故无条件按整块键盘高度平移会把
 * 稀疏会话（提示符在首行）整体推出屏幕上边界，终端区表现为全空（实测提示符由
 * y=134 落到 y=−686）。本函数返回的量等价于 Termux `adjustResize` 会砍掉的那些行高：
 * 放得下的内容每个像素都留在原处，放不下的才上移，且上移后末行恰好贴在键栏顶边。
 *
 * 备用屏（helix/vim 等全屏 TUI）恒占满视口，任何位移都会把应用顶部推出屏幕，
 * 且位移后的视觉行与触摸换算行错位；此时位移恒为 0（顶部保持可见，触摸对齐恢复）。
 * 主屏公式与行为不变。
 *
 * @param contentBottomPx 内容下沿像素（视口最后一个有内容的行的下沿，视口全空为 0）
 * @param surfaceHeightPx Surface 布局高度（容器高度，键栏覆盖其底部）
 * @param modifierBarHeightPx 键栏高度（网格已按同一口径预留）
 * @param imeBottomPx 键盘遮挡高度（已扣除被 `navigationBarsPadding` 消费的系统导航条）
 * @param isAltScreenActive 备用屏是否激活（全屏 TUI 应用）
 *
 * 上界取 `imeBottomPx`：位移超过键盘高度会在键盘上方留下一段终端背景空隙。
 * 网格已保证内容下沿不超过网格高度，故该上界在正常路径上恒不生效，
 * 只在字号变化瞬间（行数尚未随新行高重算）收敛位移。
 *
 * 位移把网格**顶部**若干行推出屏幕上边界（见 [TerminalSurface] 的显出量处理）：被推出
 * 的行不在上滑行程里，只能由边界外的过度滑动按像素回收位移看回来。
 *
 * 显出量按「内容末行」与「光标所在行」中较低的一行计算：光标常落在最后一个有字行的
 * **下一行**（提示符整行占满、或命令行结尾换行后），只按内容末行算出的位移会把光标
 * 留在键盘后面——现象就是「prompt 占满整行时输入法光标不可见、没有剩余空间」。
 */
internal fun computeImeSurfaceShift(
    contentBottomPx: Int,
    surfaceHeightPx: Int,
    modifierBarHeightPx: Int,
    imeBottomPx: Int,
    cursorBottomPx: Int,
    isAltScreenActive: Boolean = false,
): Int {
    if (isAltScreenActive) return 0
    val visibleContentPx = surfaceHeightPx - modifierBarHeightPx - imeBottomPx
    // 光标行**低于**内容末行时取光标行；取不到光标（负值/0）时退化为只看内容末行。
    val revealBottomPx = maxOf(contentBottomPx, cursorBottomPx)
    return (revealBottomPx - visibleContentPx).coerceIn(0, imeBottomPx)
}

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
     * 必须在每次 attach 时重建（见 [TerminalSurface.onAttachedToWindow]）：detach 会停掉它们，
     * 而视图复用不重走 [TerminalSurface.attachViewModel]。
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
        ViewCompat.setOnApplyWindowInsetsListener(surface) { _, insets ->
            // 切后台期间 MUST NOT 采纳 insets：`ON_PAUSE` 自己调了
            // `hideSoftInputFromWindow`（平台也会收起输入法），派发回来的高度是 0，
            // 照单收下就会把位移在用户看不见时归零。而开孔遮罩与原生保留的最后一帧
            // 都是按**离开时**的位移拍下的，回前台的首帧因此画在错误位置上，等输入法
            // 重新弹出再跳一次——即「输入法弹出时切后台返回终端闪烁」。
            //
            // 回前台的恢复期同样 MUST NOT 把「输入法已隐藏」当成用户收起：窗口重新
            // 可见的那一拍先派发的是隐藏态，而键盘要靠 [onWindowShown] 显式要回来，
            // 此间归零就是同一处闪烁。真隐藏由恢复超时兜底（见
            // [armKeyboardRestoreDeadline]），不会让位移无限期停在离开时的值。
            if (surface.viewModel?.runtime?.isBackgroundPaused == true) return@setOnApplyWindowInsetsListener insets
            if (pendingKeyboardRestore && !insets.isVisible(WindowInsetsCompat.Type.ime())) {
                return@setOnApplyWindowInsetsListener insets
            }
            adoptImeInsets(insets)
            insets
        }
    }

    /**
     * 采纳一次 insets 派发：更新遮挡高度、键栏上移、网格重排与可见性 FLIP。
     *
     * 单独成函数是为了让「恢复键盘超时」能把一次性的真实 insets 送进同一条路径
     * （[armKeyboardRestoreDeadline]），口径与派发完全一致。
     */
    private fun adoptImeInsets(insets: WindowInsetsCompat) {
        val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
        if (pendingKeyboardRestore && imeVisible) {
            // 键盘真的回来了：恢复期结束，此后一切按派发为准。
            pendingKeyboardRestore = false
            cancelKeyboardRestoreDeadline()
        }
        applyImeInset(
            maxOf(
                insets.getInsets(WindowInsetsCompat.Type.ime()).bottom -
                    insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom,
                0,
            ),
        )
        // 键盘的显示/隐藏 FLIP 才关闭选区手柄与上下文菜单——显示时定位的弹窗
        // 绝不会相对滚动而陈旧。以 FLIP（而非每像素变化）为闸门很重要：显示/隐藏
        // 动画每帧都发出 insets，逐帧清除会抹掉动画期间做出的选择。
        //
        // 必须在**本监听器**里做而不能靠 `onApplyWindowInsets`：框架在视图装了就
        // `OnApplyWindowInsetsListener` 时只调它，不再调用 `onApplyWindowInsets`
        // （ViewCompat 的 wrapper 即如此实现），放在重写方法里等于静默失效。
        setImeVisible(imeVisible)
    }

    /** 采纳一个遮挡高度（px）：位移、键栏与网格三条路径的唯一入口。 */
    private fun applyImeInset(reserved: Int) {
        if (reserved == imeInsetPx) return
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

    /** 键盘可见性 FLIP：由显示转隐藏、或反之，才清掉选区与菜单。 */
    private fun setImeVisible(visible: Boolean) {
        if (visible == lastImeVisible) return
        lastImeVisible = visible
        surface.viewModel?.clearSelection()
        surface.selectionHandles.hideSelectionHandles()
        surface.hideSelectionMenu("imeVisibilityChanged")
    }

    /** 窗口不可见（切后台）时的键盘可见性：回前台据此恢复，见 [onWindowShown]。 */
    private var imeVisibleAtWindowHide = false

    /**
     * 正在等键盘回来：期间忽略「输入法已隐藏」的 insets 派发（理由见监听器）。
     *
     * 只在 [onWindowShown] 置位，在输入法真的可见或恢复超时后清除；置位、清除与
     * 读取都在主线程（窗口可见性、焦点与 insets 派发同为主线程）。
     */
    private var pendingKeyboardRestore = false

    /** 在途的恢复超时（[armKeyboardRestoreDeadline]）；null 表示没有在等。 */
    private var keyboardRestoreDeadline: Runnable? = null

    /**
     * 宿主窗口不可见（切后台）：快照键盘可见性。
     *
     * 必须早于平台收起输入法后派发的隐藏态 insets，故挂在窗口可见性变化上而不是
     * `ON_PAUSE`：后者在组合层，时机不保证早于那次派发。
     */
    internal fun onWindowHidden() {
        imeVisibleAtWindowHide = lastImeVisible
    }

    /**
     * 宿主窗口重新可见：离开时键盘在显示就显式要求它回来。
     *
     * 平台**不保证**回前台自动还回键盘：同进程实测既有自动恢复（`fromIme=true`），
     * 也有一路不恢复直到用户轻点（`fromIme=false`）。后者正是「切后台返回终端
     * 闪烁」的成因：位移按隐藏态归零、内容下移，用户轻点后键盘弹出、内容再上移。
     */
    internal fun onWindowShown() {
        if (!imeVisibleAtWindowHide || !surface.softKeyboardEnabled) return
        imeVisibleAtWindowHide = false
        pendingKeyboardRestore = true
        if (surface.hasWindowFocus()) requestKeyboardRestore()
    }

    /** 窗口重新取得焦点：`show` 只在窗口有焦点时被平台接受。 */
    internal fun onFocusGained() {
        if (pendingKeyboardRestore) requestKeyboardRestore()
    }

    private fun requestKeyboardRestore() {
        // 已请求过（焦点与可见性两条路径都会到）就不再排一次超时、也不再 show 一次。
        if (keyboardRestoreDeadline != null) return
        armKeyboardRestoreDeadline()
        LogUtil.debug(TerminalSurface.TAG, "restoreKeyboard: show(Type.ime())")
        // 平台的 WindowInsetsController 收平台 `WindowInsets.Type` 常量，
        // 与 TerminalSurface/TerminalScreen/SurfaceInput 的同一路径一致。
        surface.windowInsetsController?.show(Type.ime())
    }

    /**
     * 恢复键盘的等待上限：超过它仍未见输入法可见（平台拒收或被切到别的输入法），
     * 按真实 insets 重新采纳——位移不能无限期停在离开时的值。
     */
    private fun armKeyboardRestoreDeadline() {
        cancelKeyboardRestoreDeadline()
        keyboardRestoreDeadline =
            Runnable {
                keyboardRestoreDeadline = null
                if (!pendingKeyboardRestore) return@Runnable
                pendingKeyboardRestore = false
                LogUtil.debug(TerminalSurface.TAG, "restoreKeyboard: timeout, 按隐藏态采纳")
                // 没有输入法可见的派发：键盘确实没回来，按隐藏态收尾。不读
                // `rootWindowInsets`——那是被本类取代的轮询通道（见 displacements 规范）。
                applyImeInset(0)
                setImeVisible(false)
            }.also { surface.postDelayed(it, KEYBOARD_RESTORE_TIMEOUT_MS) }
    }

    private fun cancelKeyboardRestoreDeadline() {
        keyboardRestoreDeadline?.let { surface.removeCallbacks(it) }
        keyboardRestoreDeadline = null
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
     * 内容下沿、单元格度量、备用屏状态（后三者由 [observeInputs] 订阅）、
     * 视口尺寸（[TerminalSurface.onSizeChanged]，旋转时 Activity 不重建）、滚动位置
     * （[TerminalSurface.applyScrollOffset]，见 [imeShiftRevealPx]）。
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
        LogUtil.debug(
            TerminalSurface.TAG,
            "applyImeShift: $appliedImeShiftPx -> $shift (capacity=$capacity inset=$imeInsetPx)",
        )
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
        pendingKeyboardRestore = false
        imeVisibleAtWindowHide = false
        cancelKeyboardRestoreDeadline()
        cancelGridResize()
        // 防抖被取消时它领的那次渲染暂停必须一并归还：`cancelGridResize` 只撤
        // runnable，凭证仍在本类手里。不归还则持有者数停在 1，共享渲染器被永久
        // 暂停（终端全黑）。归还幂等——已被 `cancelDebounceHolds` 还过的那次是空操作。
        releaseGridResizePause()
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

    private companion object {
        /**
         * 恢复键盘的等待上限（ms）。
         *
         * 输入法弹出动画约 250ms，留出平台排队余量；超过它即认为键盘不会回来，
         * 按真实 insets 重新采纳位移（见 [armKeyboardRestoreDeadline]）。
         */
        const val KEYBOARD_RESTORE_TIMEOUT_MS = 700L
    }
}
