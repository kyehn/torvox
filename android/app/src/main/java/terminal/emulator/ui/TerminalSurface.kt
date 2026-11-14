package terminal.emulator.ui

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.RectF
import android.util.AttributeSet
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PointerIcon
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowInsets.Type
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.Magnifier
import terminal.emulator.TerminalViewModel
import terminal.emulator.bridge.Bridge
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.ModifierState
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.DEFAULT_GRID_COLS
import terminal.emulator.runtime.DEFAULT_GRID_ROWS
import terminal.emulator.runtime.InputBatchBuffer
import terminal.emulator.runtime.LogUtil
import java.nio.ByteBuffer
import kotlin.math.roundToInt

class TerminalSurface
@JvmOverloads
constructor(context: Context, attrs: AttributeSet? = null, defStyleAttr: Int = 0) :
    SurfaceView(context, attrs, defStyleAttr),
    SurfaceHolder.Callback2 {

    override fun onDetachedFromWindow() {
        // 先落旗再拆窗口：平台在 onDetachedFromWindow 回调**内部**仍报告
        // `isAttachedToWindow == true`（AttachInfo 尚未清空），弹窗创建处只凭它守卫会
        // 放行——新窗口随后挂在一个正在销毁的 Activity 上，框架报
        // `WindowLeaked: Activity ... has leaked window PopupWindow$PopupDecorView`
        // 并把窗口永久留在屏幕上（实测 88 次泄漏，泄漏的是铺满全屏的手柄覆盖层；
        // 留在屏幕上的覆盖层会吃掉后续用例的触摸，见 [reshowSelectionHandles]）。
        viewDetached = true
        super.onDetachedFromWindow()
        // 强制隐藏输入法：软键盘打开时可能在旋转或返回键期间发生 detach。
        // 不做此步，键盘会残留显示在已销毁的 Activity 窗口之上（键盘卡死问题）。
        try {
            val inputMethodManager =
                context.getSystemService(
                    Context.INPUT_METHOD_SERVICE,
                ) as InputMethodManager
            inputMethodManager.hideSoftInputFromWindow(windowToken, 0)
        } catch (exception: Exception) {
            // 视图已拆除时窗口令牌可能已失效——属正常退化，记 debug 便于事后核对
            // 「键盘是否真的隐藏了」（这一步失败正是键盘残留显示在已销毁窗口之上的
            // 成因，完全静默会让这条线索无从追查）。
            LogUtil.debug(TAG, "detach 时隐藏输入法失败", exception)
        }
        // 清除 fling 滚动结束标记与渲染恢复回调：两者都是捕获本视图的 postDelayed
        // runnable；detach 之后它们会在已销毁的窗口上触发（且重复 fling 会叠加多份）。
        // fling 动画经 postOnAnimation 按 vsync 驱动；在 detach 时显式取消，
        // 使没有任何一步在已销毁的窗口上触发。
        input.stopFlingAnimation()
        pendingUnpauseRunnable?.let { removeCallbacks(it) }
        pendingUnpauseRunnable = null
        startupKeyboardRunnable?.let { removeCallbacks(it) }
        startupKeyboardRunnable = null
        // 两个防抖的 resume 回调同样持有本视图：detach 恰在防抖窗内则
        // pause 之后 resume 丢失，渲染永久暂停。直接取消会吞掉配对的 resume
        // ——先把持有的暂停全部归还（计数归零），再取消回调。
        // 只归还自己持有的，不碰全局暂停标志：surfaceDestroyed 刚装上的暂停
        // 与设置页的暂停都不是本视图的，见 cancelDebounceHolds 的说明。
        cancelDebounceHolds()
        // 输入法网格防抖也必须取消，而不只是作废凭证（`forceResumeRendering` 对它
        // 刻意只清凭证，因为那条 runnable 还要执行 `applyGridResize`）。
        // detach 场景不同：insets 派发已经停摆、遮挡高度刚被归零，
        // 那个 runnable 若在 48ms 后照常跑，就会拿**旧视图**的 surfaceWidth/Height
        // 配**已归零**的扣减量向运行期下发一次 resize，把 PTY 撑回被键盘遮住的高度；
        // 视图已被丢弃（Compose 换 key）时它甚至永不 attach，等于凭空改写网格。
        // 同理必须停掉平移量的订阅源：它们跑在 viewModelScope 上（跟着宿主而非视图），
        // 保留会让旧视图被协程钉住，且 detach 后的备用屏翻转仍会命中
        // scheduleImeGridResize —— 此时 View.postDelayed 落进 mRunQueue，只有重新
        // attach 才被 drain，被丢弃的旧视图永不 attach，于是配对的 setRenderPaused(false)
        // 永不执行，共享渲染器被永久暂停（终端全黑）。与 pendingSurfaceResize 同类。
        //
        // 位移量与扣减量归零：新视图的首次 insets 派发之前不得保留旧值，
        // 否则换视图 / Activity 重建后终端会被上一份键盘高度上移（旋转不重建 Activity，
        // 由 `onSizeChanged` → `applyImeShift` 按新视口高度覆盖，不走这条路径）。
        ime.reset()
        if (translationY != 0f) translationY = 0f
        // 运行期那份也必须归零：它只由本视图的 insets 派发写入，而新视图（或复用后
        // 的本视图）的首次派发若发生在键盘已收起时，算出的 `reserved` 就是 0、与它
        // 自身的 `imeInsetPx` 相等 → 被 `!=` 门控挡下、不再发布，运行期就永久保留
        // 旧的键盘高度（键栏错位、备用屏网格塌缩且不自愈）。
        //
        // 归零 MUST 核对发布者而不是数值：`key(surfaceKey)` 换视图时新旧 detach/attach
        // 的先后没有保证，而同一窗口里两个视图看到的输入法高度相同、按值判断会误判成
        // 「我还是最后发布者」，把新视图刚发布的真实高度覆盖成 0 且此后永不修复。
        viewModel?.runtime?.clearImeInsetPxIfOwnedBy(this)
        // 关闭浮动的选区 UI：action mode、选区手柄弹窗与放大镜都持有系统窗口，
        // 会在视图 detach 后继续让本视图（及整条 viewModel 链）存活
        // ——与上方的 runnable 同属一类泄漏。Surface 拆除路径也会调用它，
        // 但 detach 可以不伴随 Surface 销毁发生（重组时 Compose 替换视图）。
        //
        // `restoreHandles = false` MUST：默认值会在菜单关闭后按当前选区重建手柄窗口，
        // 而此刻视图正在拆除——`isAttachedToWindow` 在 onDetachedFromWindow 内部仍为 true
        // （框架先回调、后清 AttachInfo），守卫拦不住，于是 Activity 销毁时多出一个
        // System 窗口：`WindowLeaked: Activity ... has leaked window PopupDecorView` +
        // `Input channel object '... PopupWindow' was disposed without first being removed`。
        // 每个 detach 一份，整套仪器用例跑下来累积成百个悬空窗口与输入通道。
        selectionHandles.hideSelectionHandles()
        hideSelectionMenu("detach", restoreHandles = false)
        dismissMagnifier("onDetachedFromWindow")
        // 停止 PtyWriter 发送线程：视图正在被销毁（Activity 重建、返回键），
        // 而新的 TerminalSurface 会构建新的 InputBatchBuffer。
        // 不做此步，每次重建都会泄漏一个守护线程，它经 sink 闭包钉住整条视图链。
        inputBatchBuffer.close()
        // 摘掉排队钩子：批缓冲已关闭，留着会让此后的按键写入被拒并逐条记警告，
        // 而直写本就是合法路径（视图不在前台时）。
        //
        // 只摘自己装的那一份：Activity 重建时新旧视图短暂共存，旧视图 detach 可能晚于
        // 新视图 attach；无条件按「当前活动会话」的 bridge 清空会摘掉新视图刚装的钩子，
        // 硬件键/鼠标退回主线程同步写 PTY（PTY 满时每次按键冻结 UI 数秒）。
        val binding = ptyWriteQueueBinding
        if (binding != null && ptyWriteQueueBridge?.ptyWriteQueue === binding) {
            ptyWriteQueueBridge?.ptyWriteQueue = null
        }
        ptyWriteQueueBridge = null
        ptyWriteQueueBinding = null
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        // 与 [viewDetached] 成对：重新 attach 后才允许再建弹窗窗口。
        viewDetached = false
        // 同一实例可被重新 attach（见 inputBatchBuffer 注释）：重建已关闭的批缓冲。
        inputBatchBuffer = newInputBatchBuffer()
        // 排队钩子 MUST 随 attach 恢复：它在 detach 时被摘掉（批缓冲已关闭，留着会让
        // 此后的按键写入被拒），而视图复用不重走 attachViewModel。漏恢复的后果是
        // 硬件键与鼠标退回同步 feedPty——主端满时每次按键最多冻住 UI 线程 5s。
        bindActivePtyWriteQueue()
        // 订阅必须随 attach 恢复，不能只在 attachViewModel 里建一次：detach 会停掉它们
        // （见 onDetachedFromWindow 的清理），而视图复用时不会重新走 AndroidView.factory，
        // 于是 altScreenActiveFlow / lastContentRowFlow / cellMetricsFlow 三条输入全部失联
        // ——网格再也不重排、主屏位移不再跟随内容。
        viewModel?.let { ime.observeInputs(it) }
    }

    fun setDimensions(rows: Int, cols: Int) = resizeManager.setDimensions(rows, cols)

    /** 直接转发：选区手柄弹窗由 [SelectionHandles] 实现。 */
    fun showSelectionHandles(startRow: Int, startCol: Int, endRow: Int, endCol: Int, themeFgColor: Int) =
        selectionHandles.showSelectionHandles(startRow, startCol, endRow, endCol, themeFgColor)

    /** 直接转发：选区手柄弹窗由 [SelectionHandles] 实现。 */
    fun hideSelectionHandles() = selectionHandles.hideSelectionHandles()

    /** 直接转发：选区上下文菜单由 [SelectionMenu] 实现。 */
    fun showSelectionMenu(pasteOnly: Boolean) = selectionMenu.showSelectionMenu(pasteOnly)

    /** 直接转发：选区上下文菜单由 [SelectionMenu] 实现。 */
    fun hideSelectionMenu(reason: String, restoreHandles: Boolean = true) =
        selectionMenu.hideSelectionMenu(reason, restoreHandles)

    companion object {
        internal const val TAG = "TerminalSurface"

        /** 度量未就绪时的兜底网格上界：手机上的终端最多几百列。 */
        internal const val MAX_FALLBACK_GRID_SIZE = 512

        private const val SUPPRESS_GRACE_PERIOD_NS = 50_000_000L
        private const val DRAWER_CLOSE_TAP_GRACE_NANOS = 350_000_000L

        /**
         * 稳定窗：输入法动画或 Surface 尺寸抖动期间逐帧到达的事件，攒够 3 帧（≈48ms）
         * 无变化才认为到达稳定。
         *
         * 同时用于两处：交换链重配置的防抖（防抖窗内暂停渲染）与输入法网格重排的防抖。
         * 3 帧是取舍：更短则在候选栏闪烁时会漏掉最终的稳定高度，更长则键盘弹出后
         * 全屏 TUI 多等几帧才重排。
         */
        internal const val IME_RESIZE_DEBOUNCE_MS = 48L
        private const val SCROLLBACK_QUERY_THROTTLE_NANOS = 100_000_000L // 10 Hz

        private const val FALLBACK_CELL_WIDTH = 8f
        private const val FALLBACK_CELL_HEIGHT = 16f
    }

    internal fun getAccentColor(): Int = viewModel?.runtime?.accentColor ?: 0xFF2196F3.toInt()

    @Volatile
    internal var viewModel: TerminalViewModel? = null

    /** 绑定宿主视图模型：Surface 回调内的全部运行期调用经此进入。 */
    fun attachViewModel(viewModel: TerminalViewModel) {
        this.viewModel = viewModel
        // 选区菜单粘贴经批缓冲异步写（N1-26）：与长按粘贴同一出口，
        // 避免主线程逐块同步写 PTY。
        viewModel.pasteSink = { sessionId, data ->
            // 「粘贴内容没到 shell」必须能从日志判定：本行记入队，[InputBatchBuffer]
            // 记录刷写失败或丢弃，`executePaste` 记跳过——三段合起来覆盖整条链路。
            LogUtil.debug("TerminalSurface", "粘贴入队 ${data.size} 字节 -> 会话 $sessionId")
            inputBatchBuffer.write(data, sessionId)
        }
        // 硬件按键与鼠标事件同样排到批缓冲（理由见 [bindActivePtyWriteQueue]）。
        bindActivePtyWriteQueue()
        ime.observeInputs(viewModel)
    }

    @Volatile internal var rows: Int = DEFAULT_GRID_ROWS

    @Volatile internal var cols: Int = DEFAULT_GRID_COLS
    internal var surfaceWidthPixels: Int = 0
    internal var surfaceHeightPixels: Int = 0
    internal var isScrolling: Boolean = false
    internal var scrollAccumulatorPx: Float = 0f

    @Volatile internal var scrollOffset: Int = 0

    /**
     * 上次重基时的回滚长度：内容在底边增长了多少，滚动偏移就跟着重基多少。
     * -1 = 尚未重基过（首个基线无从比较，只记录不调整）。
     */
    private var scrollbackLengthAtRebase: Int = -1

    /**
     * 把滚动偏移重基到当前真值口径（离实时视图多少行），返回当前回滚长度。
     *
     * 浏览回滚期间新输出把活动区底边推远，同一处内容离底边更远了，偏移必须跟着
     * 增长，否则会漂移：回到底边时按旧值算出的目标停在底边之上（新输出落在视口
     * 之外，终端看上去「死了」），滚到回滚区顶端时也会多滚出实际可滚的行数。
     *
     * 重基量取回滚长度的增长（内容在底边增长的那部分），不直接抄原生发布的真值：
     * 真值同时含「视口自己移动」的部分，直接抄会把尚未落地的那一步抹掉——滚动丢步、
     * 手感发黏。重基只挪基线，用户已请求的那部分原样保留。
     *
     * 基线取运行时那份（见 [getScrollOffset]）：运行时是真源，新输出在渲染线程把
     * 偏移复位为 0（自动贴底），本地字段只是手势期间的镜像。
     */
    internal fun rebaseScrollOffset(): Int {
        val scrollbackLen = currentScrollbackLength()
        val previousLen = scrollbackLengthAtRebase
        if (previousLen < 0) {
            scrollbackLengthAtRebase = scrollbackLen
            return scrollbackLen
        }
        val base = viewModel?.runtime?.activeSessionScrollOffset() ?: scrollOffset
        val rebased = rebasedScrollOffset(base, scrollbackLen, previousLen)
        scrollbackLengthAtRebase = scrollbackLen
        if (rebased != scrollOffset) {
            applyScrollOffset(rebased)
        }
        return scrollbackLen
    }

    // 回滚长度缓存：正常情况下直接读渲染线程随帧发布的原子量（零等待），
    // 只有首帧尚未发布时才回退到同步 JNI 查询；回退路径仍按 ~10Hz 节流，
    // 因为 VT 线程忙于解析大块写入时那次查询最多可阻塞约 1s。
    @Volatile private var cachedScrollbackLength: Int = 0

    private var lastScrollbackQueryNanos: Long = 0L

    /** 组字光标锚点最后一次查询的时刻，用于给同步 VT 查询限频。 */
    internal var lastCursorAnchorQueryNanos: Long = 0L

    var touchEnabled: Boolean = true
        set(value) {
            field = value
            isFocusable = value
            isFocusableInTouchMode = value
            if (!value) {
                clearFocus()
            }
        }

    /** 让原生按当前搜索状态重建视口高亮并绘制：命中区间、选中态与颜色都在原生侧。 */
    fun applySearchHighlights(scrollOffset: Int) {
        val bridge = viewModel?.runtime?.bridge() ?: return
        bridge.applySearchHighlights(scrollOffset)
        viewModel?.runtime?.forceRender()
    }

    fun clearSearchHighlights() {
        val bridge = viewModel?.runtime?.bridge() ?: return
        bridge.clearSearchHighlights()
        // 清除高亮后强制重绘，使反色立即消失而不是残留一帧。
        viewModel?.runtime?.forceRender()
    }

    private var magnifier: Magnifier? = null

    /** 在（rawX, rawY）处弹出放大镜：右键选词的放大镜由输入处理器触发。 */
    internal fun showMagnifierAt(rawX: Float, rawY: Float) {
        try {
            magnifier = magnifier ?: Magnifier.Builder(this).build()
            magnifier?.show(rawX, rawY)
        } catch (exception: Exception) {
            LogUtil.warn(TAG, "magnifier show failed (non-critical)", exception)
        }
    }

    /**
     * 收起放大镜并丢弃引用。
     *
     * `Magnifier` 是系统窗口级组件：只 `show()` 不 `dismiss()` 会让它常驻屏幕，
     * 直到本视图 detach。收起的调用点因此必须与显示点**成对**——见
     * [showMagnifierAt] 与鼠标右键分支。
     */
    internal fun dismissMagnifier(reason: String) {
        try {
            magnifier?.dismiss()
        } catch (exception: Exception) {
            LogUtil.warn(TAG, "magnifier dismiss failed ($reason, non-critical)", exception)
        }
        magnifier = null
    }

    /**
     * 写入视口滚动位置并向宿主通报。位移的显出量上界依赖滚动位置，故同步刷新位移。
     */
    internal fun applyScrollOffset(target: Int) {
        if (target == scrollOffset) return
        scrollOffset = target
        ime.applyImeShift()
        onScrollChanged?.invoke(target)
    }

    /**
     * 把一次滚动增量落到视图：偏移、位移显出量、渲染器亚行余量。
     *
     * 拖动与惯性共用这一条管道（见 [applyScrollDistance]），两者的区别只在增量
     * 从哪来——手指还是 `android.widget.OverScroller`。分成两份实现时惯性那份会漏掉边界丢弃与
     * 显出量，内容按整行跳（STATUS ⑩）。
     */
    internal fun applyScrollStep(step: ScrollStep, scrollbackLen: Int) {
        scrollAccumulatorPx = step.newAccumulatorPx
        ime.imeShiftRevealPx = step.revealPx
        if (step.newOffset != scrollOffset) {
            applyScrollOffset(step.newOffset)
        } else {
            // 偏移未变，但显出量可能刚在回滚区顶端之外被消费/回收。
            ime.applyImeShift()
        }
        // 逐像素余量：把亚行累加器镜像给渲染器，使内容在行内跟随手指。
        // 以非空回滚为条件——没有历史时任何偏移都会露出空白。
        // setScrollRemainderPx 已含 notifyRender，不再额外 forceRender
        // （双重唤醒致手势期间渲染线程空转，滚动卡顿）。
        if (scrollbackLen > 0) {
            viewModel?.runtime?.setScrollRemainderPx(scrollAccumulatorPx)
        }
    }

    /**
     * 无条件恢复渲染，并作废两个在途防抖持有的暂停。
     *
     * 用于「我们等的那件事已经发生，不必再等」的入口：交换链重配置完成
     * （[ResizeManager.applyResizeNormal]）、Surface 重新交付（[surfaceCreated]）。
     * 这些入口本就必须立刻出一帧（否则黑屏），此时继续挂着防抖暂停只会让
     * 本已就绪的缓冲不显示。
     *
     * 交换链防抖 MUST 一并取消：它等的正是「尺寸稳定」，而本函数的前提就是稳定已达成。
     * 输入法网格防抖 MUST NOT 取消——它的动作（[ResizeManager.applyGridResize]）不碰
     * 交换链；改为只作废它的凭证，让 runnable 照常执行（陈旧归还已是空操作）。
     */
    internal fun forceResumeRendering() {
        cancelDebounceHolds()
        // 立刻恢复渲染：本函数的所有调用点都刚把可用 Surface 挂上原生，故「Surface
        // 销毁」那次暂停已由挂载门闸解除。是否真的恢复由账本计数决定（设置页仍在
        // 展示时计数非零，仍保持暂停）：那是界面层的独立意图，终端视图在设置覆盖层
        // 之下仍会收到尺寸变化。
        //
        // 「切后台」那次暂停不在此归还：它只能由「回到前台」这一转场解除。
        // 本函数的调用点（Surface 重新交付、尺寸防抖到期）可能在应用仍处于后台时
        // 到达——平台收走原生窗口后宿主会换一个新 SurfaceView，新窗口交付时若把
        // 后台暂停一并还掉，渲染线程就重新在后台对着正在消亡的队列取帧
        // （ERROR_SURFACE_LOST_KHR，切回前台后终端永久黑屏）。
    }

    /**
     * 取消两个在途防抖并归还其持有的渲染暂停。
     *
     * 切后台必须调它：在途防抖仍会到期执行——它经 `applyResizeNormal` 恢复渲染，
     * 于是刚装上的后台暂停被自己的防抖撤掉，渲染线程转去已在被系统回收的
     * BufferQueue 上取帧（ERROR_SURFACE_LOST_KHR → 返回后永久黑屏）。
     *
     * 与 Surface 是否在切后台被拆除无关：平台在切后台后会拆掉本应用窗口的
     * surface（转场动画结束后立即拆除），SurfaceView 的 surface 随之被拆；防抖
     * 同样可能在后台到期。
     */
    fun cancelResizeDebounces() {
        cancelDebounceHolds()
        ime.cancelGridResize()
    }

    private fun cancelDebounceHolds() {
        // 交换链尺寸防抖与其凭证由 [ResizeManager] 持有：取消与归还都在那里，
        // 本视图只负责输入法网格防抖这一路。
        resizeManager.cancelPendingSurfaceResize()
        ime.releaseGridResizePause()
    }

    var onScrollChanged: ((offset: Int) -> Unit)? = null
    var onScrollingStateChanged: ((isScrolling: Boolean) -> Unit)? = null
    var onZoomChanged: ((fontSizeSp: Float) -> Unit)? = null

    // ⑥ 实时缩放预览：双指手势期间每秒数次以插值字号触发，渲染器无需网格 resize 即可跟随。
    // 手势终结时以稳定尺寸调用一次 onZoomChanged（完整应用 + 网格重排）。
    var onZoomPreview: ((fontSizeSp: Float) -> Unit)? = null

    var drawerOpen: Boolean = false
        set(value) {
            val wasOpen = field
            field = value
            if (value) {
                selectionHandles.hideSelectionHandles()
            } else if (wasOpen) {
                // ModalNavigationDrawer 的遮罩轻击会关闭抽屉，
                // 但在关闭动画期间该轻击可能穿透到 TerminalSurface
                // ——终端轻击会清除选区，故关闭抽屉会静默抹掉活跃的文本选区。
                // 在抽屉关闭动画时长（[DRAWER_CLOSE_TAP_GRACE_NANOS]）内抑制「轻击清除」，
                // 使选区能存活抽屉自身的关闭手势。
                armInputSuppression(DRAWER_CLOSE_TAP_GRACE_NANOS)
            }
        }

    private var cachedCellWidth: Float = FALLBACK_CELL_WIDTH
    private var cachedCellHeight: Float = FALLBACK_CELL_HEIGHT

    val cellWidth: Float
        get() {
            // 字体度量的单元格宽度（runtime.cellWidth = 原生字体管线 cell_metrics × 密度）。
            // 命中测试刻意不用 surface÷cols 比值：渲染器按字体单元格尺寸绘制字形并拉伸网格，
            // 故用 surface 除以网格会重复计入该拉伸，使长按落到错误的单元格
            // （回归已在此修复：surface÷grid 比值会自我放大
            // ——更宽的单元格缩小网格，而这又进一步加宽单元格）。
            val viewModelCellWidth = viewModel?.runtime?.cellWidth ?: 0f
            if (viewModelCellWidth > 0f) {
                cachedCellWidth = viewModelCellWidth
                return viewModelCellWidth
            }
            return cachedCellWidth
        }

    val cellHeight: Float
        get() {
            // 字体度量的单元格高度——见上方 cellWidth。
            val viewModelCellHeight = viewModel?.runtime?.cellHeight ?: 0f
            if (viewModelCellHeight > 0f) {
                cachedCellHeight = viewModelCellHeight
                return viewModelCellHeight
            }
            return cachedCellHeight
        }

    @Volatile internal var isPaused = false

    /**
     * 视图已开始（或已完成）离开窗口树。
     *
     * 弹窗创建不能用 `isAttachedToWindow` 判活：平台在 onDetachedFromWindow 回调**内部**
     * 仍报告 true（AttachInfo 先回调后清空），于是拆除过程中建出的窗口会挂在一个正在
     * 销毁的 Activity 上——框架报 `WindowLeaked: ... has leaked window PopupDecorView`
     * 并把窗口永久留在屏幕上（实测 88 次；铺满全屏的手柄覆盖层留在屏幕上后，
     * 后续用例的触摸会被它吃掉）。本旗在回调第一条语句落位，attach 时清除。
     */
    internal var viewDetached = false

    @Volatile internal var suppressUntilNanos = 0L

    /**
     * 开一个**有界**的输入抑制窗口：刚发生窗口/抽屉切换时，紧随其后的输入可能是切换
     * 手势的余波（抽屉遮罩轻击穿透、失焦瞬间的收尾提交），据此拒绝一小段。
     *
     * 已开着的窗口**不延长**——连续多次切换只应付一个窗口的代价。此前各处直接写
     * `suppressUntilNanos = now + window`，焦点/抽屉事件密集时抑制期会被反复续命，
     * 输入被连续拒绝直到事件停下来为止（IME 集成用例的整个重试窗口都会被吃掉，
     * 正是 CI 上「中文提交必须被输入连接接受」反复失败的形态）。
     *
     * 本窗口只拦「切换余波」，绝不用作整段拒收：失焦本身不阻塞输入投递
     * （见 [imeConnection] 各编辑方法——平台只会在它认定的焦点视图上调用它们）。
     */
    private fun armInputSuppression(windowNanos: Long) {
        val now = System.nanoTime()
        if (now >= suppressUntilNanos) {
            suppressUntilNanos = now + windowNanos
        }
    }

    private var pendingUnpauseRunnable: Runnable? = null

    /** 当前安装 [Bridge.ptyWriteQueue] 的那条 bridge；detach 时据此只摘自己的一份。 */
    @Volatile
    private var ptyWriteQueueBridge: Bridge? = null

    /** 本视图写入该 Bridge 的具体闭包；用于区分新视图已接管的钩子。 */
    @Volatile
    private var ptyWriteQueueBinding: ((Long, ByteArray) -> Unit)? = null

    /** 直接转发：手柄拖动由 [HandleDragController] 实现。 */
    internal fun latchDragAnchor(which: HandleDrag, pointerId: Int? = null) =
        handleDragController.latchAnchor(which, pointerId)

    /** 直接转发：手柄拖动由 [HandleDragController] 实现。 */
    internal fun driveHandleDragMove(touchX: Float, touchY: Float) = handleDragController.driveMove(touchX, touchY)

    /** 直接转发：手柄拖动由 [HandleDragController] 实现。 */
    internal fun finishHandleDrag() = handleDragController.finish()

    /** 在当前选区范围上重新隐藏并显示选区手柄（拖动结束后使用，使手柄吸附到最终单元格）。 */
    internal fun reshowSelectionHandles() {
        // 拆除中绝不重建覆盖层：框架在 detach 回调内部仍报告 isAttachedToWindow=true，
        // 建出的窗口挂在正在销毁的 Activity 上并被永久留下（WindowLeaked: PopupDecorView，
        // 实测 88 次）——留在屏幕上的全屏覆盖层会吃掉后续触摸。
        if (viewDetached || !isAttachedToWindow) return
        // 拖动中绝不重建覆盖层：`hideSelectionHandles` 会 dismiss 掉承载本次按压的弹窗，
        // 窗口令牌随之销毁，平台立刻取消手势——后续 MOVE/UP 落到新窗口却已无 DOWN，
        // 表现为按住手柄一瞬即断、抬手不重锚（仪器用例 SelectionDragQuantifiedTest 判红）。
        // 抓柄路径本来就是「先 latchDragAnchor 隐藏菜单、再走 reshow」，必须在这里挡住；
        // 拖动结束时 finishHandleDrag 会自行调用本函数，届时手柄已吸附到最终单元格。
        if (handleDragController.state != HandleDrag.NONE) return
        val selection = viewModel?.state?.value?.selection
        if (selection?.start != null && selection.end != null) {
            selectionHandles.hideSelectionHandles()
            selectionHandles.showSelectionHandles(
                selection.start.row,
                selection.start.col,
                selection.end.row,
                selection.end.col,
                getAccentColor(),
            )
        }
    }

    internal val clipboardAccess = ClipboardAccess(context, tag = "Surface")
    internal val resizeManager = ResizeManager(this)
    private val imeConnection = ImeConnection(this)
    internal val selectionHandles = SelectionHandles(this)
    private val selectionMenu = SelectionMenu(this)
    internal val handleDragController = HandleDragController(this)
    internal val ime = ImeShiftController(this)
    private val input = SurfaceInput(this)

    val isSelectingText: Boolean
        get() = viewModel?.state?.value?.selection?.active == true

    init {
        holder.addCallback(this)
        // SurfaceView 在窗口中开了孔；终端内容由原生渲染器绘制到 Surface，
        // 其余一切（工具栏、覆盖层）留在普通视图层级中。
        holder.setFormat(PixelFormat.RGBA_8888)
        isFocusable = true
        isFocusableInTouchMode = true
        setWillNotDraw(false)
        // 输入法遮挡高度的唯一来源：平台 insets 派发（见 installImeInsetListener）。
        ime.installImeInsetListener()
    }

    internal var keyboardRequested = false

    /** 启动键盘弹起重试：窗口取得焦点前 `show` 会被平台丢弃，按 100ms 重试到 2s。 */
    private var startupKeyboardAttempts = 0
    private var startupKeyboardRunnable: Runnable? = null

    /**
     * 软键盘是否允许弹出（`soft-keyboard-toggle-behaviour = enable/disable`）。
     *
     * 两条路径都要拦：轻点/KEYBOARD 键不再调用 `show`，且 [onCheckIsTextEditor] 报否。
     * 后者不是冗余——平台在焦点变化时会按它**自动**弹键盘，只拦 `show` 的话，轻点里那次
     * `requestFocus` 就足以把键盘带出来。
     *
     * 默认 `true`：`show/hide` 模式下本字段从不参与判定，行为与未引入时逐字节相同。
     */
    var softKeyboardEnabled: Boolean = true

    /**
     * `hide-soft-keyboard-on-startup = false`：启动时主动请求一次输入法。
     *
     * 本函数只做「这一次请求」本身：把 [keyboardRequested] 置真后平台才会在焦点变化时
     * 按 [onCheckIsTextEditor] 自动弹出，同时自己再显式 `show` 一次——只靠自动弹出时，
     * 焦点往往早已就位、平台不会再问，键盘永远不出现。
     *
     * 不保证成功：窗口尚未取得焦点时 `show` 会被平台丢弃。该键的语义是「不要主动隐藏」，
     * 不是「强制弹出」。
     *
     * 「整个进程只请求一次」由**调用方**认领（组合层经
     * [terminal.emulator.runtime.TerminalRuntime.claimStartupKeyboardRequest]）：
     * 宿主在原生 surface 判死时会换掉
     * 整个 `SurfaceView`，新实例若把「已请求」当成没请求过，键盘便会在换视图后再次弹出
     * ——弹出的键盘顺带清掉选区与选区菜单（见 [installImeInsetListener] 的
     * `imeVisibilityChanged` 分支），用户看到的是「选中的文字突然没了菜单」。
     *
     * 认领留在调用方而非本函数，是为了让「视图从此愿意接收文本输入」这件事可以在没有
     * 运行期（因而没有会话、没有原生库）的单元测试里被验证。
     */
    fun requestKeyboardOnStart() {
        keyboardRequested = true
        val runnable =
            object : Runnable {
                override fun run() {
                    if (!softKeyboardEnabled) {
                        startupKeyboardRunnable = null
                        return
                    }
                    if (hasWindowFocus()) {
                        startupKeyboardRunnable = null
                        requestFocus()
                        windowInsetsController?.show(Type.ime())
                        return
                    }
                    if (startupKeyboardAttempts++ >= STARTUP_KEYBOARD_ATTEMPTS) {
                        startupKeyboardRunnable = null
                        return
                    }
                    postDelayed(this, STARTUP_KEYBOARD_RETRY_INTERVAL_MS)
                }
            }
        startupKeyboardRunnable = runnable
        post(runnable)
    }

    fun finishComposing() {
        imeConnection.currentInputConnection?.let { inputConnection ->
            inputConnection.finishComposingText()
        }
    }

    // 平台在焦点变化时会问这两个问题：`onCheckIsTextEditor` 为真则**自动**弹出输入法
    // （`InputMethodManager.checkFocus`），与谁调了 `show` 无关。软键盘被禁用时必须
    // 报 false——否则轻点请求焦点这一步就足以把键盘带出来，绕过禁用。
    override fun onCheckIsTextEditor(): Boolean = keyboardRequested && softKeyboardEnabled

    override fun onResolvePointerIcon(event: MotionEvent, pointerIndex: Int): PointerIcon =
        PointerIcon.getSystemIcon(context, PointerIcon.TYPE_TEXT)

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        viewModel?.runtime?.focusChange(hasFocus)
        if (!hasFocus) {
            // 失焦只据实记账，不拦输入：平台把文本送进本连接，即表明输入法
            // 正对着本视图（连接由平台按焦点选择），此时拒收会让「切回来立刻打字」
            // 丢掉最初几个字，且在无头仪器化环境下（窗口长期无焦点）会把输入
            // 整段饿死——历史上多次以「输入一直不被接受」的形式失败。
            // 真正需要拦的只是切换瞬间的余波，交由下方有界窗口处理。
            isPaused = true
            imeConnection.currentInputConnection?.let { inputConnection ->
                inputConnection.finishComposingText()
            }
            armInputSuppression(SUPPRESS_GRACE_PERIOD_NS)
        }
        if (hasFocus) {
            // 重置暂停状态，但绝不要调用 finishComposingText——输入法可能正处于
            // 组字中；在此中止会导致输入法失步，产生重复文本、多余空格或静默丢字。
            isPaused = false
            armInputSuppression(SUPPRESS_GRACE_PERIOD_NS)
        }
    }

    fun postDelayedUnpause(delayMillis: Long) {
        pendingUnpauseRunnable?.let { removeCallbacks(it) }
        pendingUnpauseRunnable =
            Runnable {
                pendingUnpauseRunnable = null
                if (!hasWindowFocus()) return@Runnable
                isPaused = false
                val viewModel = viewModel ?: return@Runnable
                val surface = holder.surface
                // Surface 就绪后按常规路径恢复：重启渲染线程并出一帧探针。
                // Surface 仍无效时留待下一次 surfaceChanged/resize。
                //
                // 新窗口尚未挂上原生时 MUST NOT 恢复：平台已废弃旧 Surface 的
                // BufferQueue，此刻恢复会让渲染线程在废弃队列上取帧
                // （ERROR_SURFACE_LOST_KHR，返回后永久黑屏）。那次暂停由门闸
                // 持有，等 surfaceCreated 挂上新窗口再解除。
                if (
                    surface.isValid &&
                    width > 0 &&
                    height > 0 &&
                    !viewModel.runtime.isAwaitingSurfaceAttach
                ) {
                    viewModel.runtime.resumeRendering()
                    viewModel.runtime.forceRender()
                }
            }
                .also { postDelayed(it, delayMillis) }
    }

    internal fun currentScrollbackLength(): Int {
        val viewModel = viewModel ?: return cachedScrollbackLength
        val bridge = viewModel.runtime.bridge() ?: return cachedScrollbackLength
        // 首选渲染线程随帧发布的原子量：与屏幕内容同源，且永不等待 VT 线程。
        // 旧路每次节流窗口到期都发同步 RPC，VT 忙于输出时会把滚动/拖柄阻塞最长 500ms。
        val rendered = bridge.renderedScrollbackLength()
        if (rendered >= 0) {
            cachedScrollbackLength = rendered
            viewModel.updateScrollbackLength(rendered)
            return rendered
        }
        // 首帧尚未被渲染线程取走：此时才退到一次同步查询（每 100ms 至多一次）。
        val now = System.nanoTime()
        if (now - lastScrollbackQueryNanos < SCROLLBACK_QUERY_THROTTLE_NANOS) {
            return cachedScrollbackLength
        }
        // 时间戳只在**查询成功**之后才推进：bridge 缺席或查询抛错时都返回陈旧缓存，
        // 此时若已消耗节流窗口，后续手势会继续锚到上一个会话的回滚长度长达 100ms。
        cachedScrollbackLength =
            try {
                bridge.scrollbackLength().also { lastScrollbackQueryNanos = now }
            } catch (error: Exception) {
                LogUtil.error(TAG, "scrollbackLength query failed", error)
                cachedScrollbackLength
            }
        viewModel.updateScrollbackLength(cachedScrollbackLength)
        return cachedScrollbackLength
    }

    /** 视口顶部显示的绝对网格行（scrollbackLength - scrollOffset）：选区矩形与拖动/光标手柄都需要的唯一来源。 */
    internal fun currentViewportTopGrid(): Int = currentScrollbackLength() - scrollOffset

    fun scrollToRow(row: Int) {
        input.stopFlingAnimation()
        val scrollbackLen = rebaseScrollOffset()
        val targetOffset = scrollOffsetForRow(row, scrollbackLen)
        if (targetOffset != scrollOffset) {
            applyScrollOffset(targetOffset)
            // 以信号（按 vsync 节奏）通知渲染线程，
            // 而不是在每个滚动事件上用同步 GPU 渲染阻塞 UI 线程。
            viewModel?.runtime?.forceRender()
        }
    }

    /**
     * 把本地滚动偏移重置为会话的偏移；会话切换时调用，使选区坐标计算不使用上一个会话的偏移。
     *
     * 回滚长度缓存必须一并失效：`currentViewportTopGrid()` = 回滚长度 - 偏移，
     * 而 `currentScrollbackLength()` 有 100ms 节流，只重置偏移会让切会话后的首个手势
     * 拿上一个会话的回滚长度算出绝对网格行（长按/拖手柄因此锚到无关的回滚区）。
     */
    fun resetScrollOffset() {
        input.stopFlingAnimation()
        lastScrollbackQueryNanos = 0L
        // 重基基线属于会话：沿用上一个会话的回滚长度，切换后的首个手势会把
        // 「两个会话回滚长度之差」当成内容增长加进偏移，锚到无关行。
        scrollbackLengthAtRebase = -1
        // 亚行余量同样属于会话：留着上一次手势的半行，新会话的首个滚动会先跳半行。
        scrollAccumulatorPx = 0f
        viewModel?.runtime?.clearScrollRemainder()
        val sessionOffset = viewModel?.runtime?.activeSessionScrollOffset() ?: 0
        if (scrollOffset != sessionOffset) {
            scrollOffset = sessionOffset
            // 会话切换连回滚长度一起换掉，位移显出量的上界随之变化（见 applyImeShift）。
            ime.applyImeShift()
        }
    }

    fun getScrollOffset(): Int {
        // 运行时为真源：新输出在渲染线程将 entry 置 0，若只读本地字段则回车后视图永不回底。
        // 手势期间本地与运行时同步更新（onScroll 双写），此处优先读运行时保证回底可见。
        return viewModel?.runtime?.activeSessionScrollOffset() ?: scrollOffset
    }

    fun getRows(): Int = rows

    /**
     * 搜索命中行居中后的视口顶行（绝对网格行）。
     *
     * 可见带由输入法遮挡、键栏占高与内容跟随键盘的上移量共同决定，折算见
     * [centerSearchViewportRow]。
     */
    fun searchViewportTop(matchRow: Int): Int {
        val runtime = viewModel?.runtime ?: return matchRow
        val cellHeight = runtime.cellHeight
        if (cellHeight <= 0f) return matchRow
        val visibleContentPx = (height - runtime.modifierBarHeightPx - ime.imeInsetPx).coerceAtLeast(0)
        val viewportRow = centerSearchViewportRow(visibleContentPx, ime.appliedImeShiftPx, cellHeight)
        return (matchRow - viewportRow).coerceAtLeast(0)
    }

    /**
     * 把搜索命中行居中到可见带。
     *
     * 只滚动不够：内容为跟随键盘整体上移时，靠近顶部的命中行即使把视口滚到顶
     * 也仍在屏幕上边界之外（屏幕坐标 = 行号 × 单元高 − 上移量，为负）。此时先
     * 把位移显出量拉满、上移量归零，再按新的可见带重算一次目标行——命中行才会
     * 真正出现在键盘上方，而不是「关闭输入法后莫名可见」。
     */
    fun centerSearchRow(matchRow: Int) {
        val runtime = viewModel?.runtime ?: return
        val cellHeight = runtime.cellHeight
        if (cellHeight <= 0f) return
        // 导航要按当前回滚长度换算目标行；缓存可能停在 100ms 节流窗口内的旧值。
        lastScrollbackQueryNanos = 0L
        scrollToRow(searchViewportTop(matchRow))
        // 滚动后上移量可能已随显出量上界变化；命中在视口内的行号 × 单元高
        // 小于上移量即落在屏幕上边界之外，滚动到顶也救不回。
        val viewportRow = matchRow - currentViewportTopGrid()
        if (ime.appliedImeShiftPx > 0 && viewportRow * cellHeight < ime.appliedImeShiftPx.toFloat()) {
            ime.imeShiftRevealPx = ime.imeShiftCapacityPx.toFloat()
            ime.applyImeShift()
            scrollToRow(searchViewportTop(matchRow))
        }
    }

    fun getCols(): Int = cols

    // 视图 detach 时 close() 会 shutdown 发送线程；同一实例再次 attach（窗口转换、
    // 分屏、Compose 重新挂载）时必须重建，否则每次 write 都抛 RejectedExecutionException
    // ——输入法输入与粘贴会在无任何症状的情况下永久失效。
    internal var inputBatchBuffer = newInputBatchBuffer()

    /**
     * 本视图的 Surface 归属凭证：一个视图实例一个。用于区分「当前持有 Surface 的视图」
     * 与「Activity 重建后仍在迟到回调的旧视图」
     * （见 [terminal.emulator.runtime.TerminalRuntime.onSurfaceDestroyed]）。
     */
    private val surfaceOwnershipToken = Any()

    /**
     * 把硬件按键与鼠标事件的写入接到批缓冲。
     *
     * 直写的 `feedPty` 在主端满时最多等 5s，会把 UI 线程冻住；共用一条出口还顺带
     * 保证与输入法写入的先后顺序（两路各写时，缓冲中待发的退格会被直写的击键越过）。
     * attach 与首次绑定都要装，故只此一处实现。
     */
    internal fun bindActivePtyWriteQueue() {
        val bridge = viewModel?.runtime?.bridge()
        val previousBridge = ptyWriteQueueBridge
        val previousBinding = ptyWriteQueueBinding
        // 只摘自己装的那一份：新视图可能已接管同一个 Bridge。
        if (previousBridge !== bridge && previousBridge?.ptyWriteQueue === previousBinding) {
            previousBridge?.ptyWriteQueue = null
        }
        ptyWriteQueueBridge = bridge
        ptyWriteQueueBinding = null
        if (bridge == null) return
        val binding: (Long, ByteArray) -> Unit = { sessionId, data ->
            inputBatchBuffer.write(data, sessionId)
        }
        bridge.ptyWriteQueue = binding
        ptyWriteQueueBinding = binding
    }

    internal fun hasActivePtyWriteQueue(): Boolean {
        val binding = ptyWriteQueueBinding ?: return false
        return ptyWriteQueueBridge === viewModel?.runtime?.bridge() &&
            ptyWriteQueueBridge?.ptyWriteQueue === binding
    }

    private fun newInputBatchBuffer(): InputBatchBuffer = InputBatchBuffer(
        flushSink = { sessionId, data ->
            // 未绑定时 `viewModel?.` 会把刷写变成无声空操作：粘贴与输入就此消失，
            // 日志里片痕不留。[InputBatchBuffer] 对「已关闭/被拒绝」都留了日志，
            // 这条同样必须留，否则「字节入队了却没到 shell」永远无法从日志判定。
            val target = viewModel
            if (target == null) {
                LogUtil.warn("TerminalSurface", "批缓冲刷写丢弃：视图模型尚未绑定 (${data.size} 字节)")
            } else {
                target.writeToPty(sessionId, data)
            }
        },
        inputSessionId = { viewModel?.runtime?.inputTargetSessionId ?: 0L },
    )

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection = imeConnection.createInputConnection(
        outAttrs,
    )

    /** 手势完成点同步亮出当前选择的菜单（控制柄同模式），Compose 侧作为同步备份。 */
    internal fun showSelectionMenuForCurrentSelection() = selectionMenu.showSelectionMenuForCurrentSelection()

    private fun modifierBitmask(event: KeyEvent): Int {
        val state = viewModel?.state?.value
        return KeyModifiers.fromKeyEvent(
            event,
            state?.ctrlState ?: ModifierState.Off,
            state?.altState ?: ModifierState.Off,
        )
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val terminalViewModel = viewModel
        // 硬件回车（包括 maestro 的 pressKey 与 adb keyevent）经
        // bridge.processKeyEvent 绕过 writeToPty
        // ——正是「输入新命令 + 回车不滚动」这一反馈的根因。
        // 共用输入驱动的贴底逻辑，使任意回车都立即贴底，
        // 且任意硬件输入都会清除 SCROLL 锁。
        if (terminalViewModel != null) {
            terminalViewModel.onUserInputForScrollSnap(keyCode in TerminalInputEncoder.enterKeyCodes)
        }
        val bridge = terminalViewModel?.runtime?.bridge()
        if (bridge != null) {
            val modifiers = modifierBitmask(event)
            val action: Int = 0 // KeyEvent.ACTION_DOWN = 0
            val success =
                bridge.processKeyEvent(
                    keyCode,
                    modifiers,
                    action,
                    event.unicodeChar,
                    // 无修饰码点：kitty 键盘协议据此区分 Shift 前后的大小写档位，
                    // Alt 组合在上游拿不到文本时也由它推导字面量。
                    event.getUnicodeChar(0),
                )
            if (success) {
                terminalViewModel.consumeOneShotModifiers()
                return true
            }
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        // 刻意不把 key-up 转发给 bridge：Bridge 的 processKeyEvent 会丢弃所有
        // 非 ACTION_DOWN 事件（在 UP 上写入会让每次击键写两遍），
        // 故它在此总是返回 false。放行给系统，
        // 使 key-up 语义（长按重复、系统手势）不被吞掉。
        return super.onKeyUp(keyCode, event)
    }

    internal fun handleKeyEvent(event: KeyEvent): Boolean = onKeyDown(event.keyCode, event)

    // ══════════════════════════════════════════════════════════════════════
    // 四、触摸事件派发
    // ══════════════════════════════════════════════════════════════════════

    override fun performClick(): Boolean = super.performClick()

    /**
     * 派发约 15 种彼此不同的手势/意图，涵盖选区、滚动、长按与硬件按键交互（实现见 [SurfaceInput]）。
     *
     * 轻击的无障碍播报由 [SurfaceInput] 在抬手时调用本视图的 [performClick]——播报时机
     * 必须在抽屉边缘手势与遮罩轻击之后（那两条抬手不属于终端），故不能把调用点挪回本
     * 函数开头。lint 的 `ClickableViewAccessibility` 只看本函数的调用图，看不到委托，
     * 故在此定点抑制：无障碍行为本身已实现。
     */
    @Suppress("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean = input.onTouchEvent(event)

    /**
     * 鼠标滚轮事件（外接鼠标/触控板）。应用启用鼠标上报时
     * （DECSET 1000/1002/1003），滚轮事件经 Ghostty 鼠标编码器编码为按钮 4/5
     * 并写入 PTY；未启用时在主屏滚动本地回滚，备用屏保持不滚动。
     */
    override fun onGenericMotionEvent(event: MotionEvent): Boolean =
        input.onGenericMotionEvent(event) ?: super.onGenericMotionEvent(event)

    // ── Surface 生命周期（SurfaceHolder.Callback；原生渲染器绘制到 Surface
    // ── —— 用 SurfaceView 而非 TextureView，因为 TextureView 的 SurfaceTexture
    // ── 会被 GL 合成器消费，在软件模拟器上阻塞 Vulkan 的 dequeueBuffer） ──

    override fun onSizeChanged(width: Int, height: Int, previousWidth: Int, previousHeight: Int) {
        super.onSizeChanged(width, height, previousWidth, previousHeight)
        if (width <= 0 || height <= 0) return
        if (width == previousWidth && height == previousHeight && previousWidth != 0) return

        surfaceWidthPixels = width
        surfaceHeightPixels = height
        resizeManager.recomputeRowsColsImmediate(width, height)
        // 同步且立即地 resize GPU 交换链，使渲染帧始终匹配新视图尺寸：
        // 若 wgpu/交换链缓冲哪怕停留在旧尺寸几帧（如输入法动画期间），
        // 陈旧缓冲就会被非均匀缩放——文字会明显拉伸/压缩。
        // 立即 resize 使缓冲与视图始终相等，彻底消除该瑕疵。
        resizeManager.applySurfaceResize(width, height)
        // 视口高度是平移量的输入之一（`computeImeSurfaceShift` 的 `surfaceHeightPx`）。
        // Activity 声明了 `configChanges` 含 orientation/screenSize，旋转不重建
        // Activity，故同一个视图实例会带着旧平移量走到新高度上。
        ime.applyImeShift()
    }

    // ── 开孔遮罩 ────────────────────────────────────────────────────────────
    //
    // 平台在切后台后会拆掉本应用窗口的 surface（转场动画结束后立即拆除），
    // `SurfaceView` 的 surface 随之被拆；回前台时交换链必须重建，那几十毫秒里
    // 窗口的透明开孔没有缓冲——用户看到的就是「切后台返回终端闪烁」。
    //
    // 遮罩只能由**本视图自己**画，且必须画在 `onDraw` 里：`SurfaceView.draw` 先给窗口
    // 打洞（`clearSurfaceViewPort`），再走 `super.draw`——也就是 `onDraw`。故 `onDraw`
    // 画的内容落在洞**之后**，盖在原生 surface 之上；组合层（窗口层）画的则相反：
    // 它在洞**之下**，一律被平台的打洞抹掉（实测组合层涂的底色只在 SurfaceView 之外的
    // 边距上可见，洞里始终是空的）。
    //
    // 盖在原生 surface 之上意味着遮罩会把终端内容一起盖住，故只在「原生尚无缓冲」
    // 这段窗口内画；遮罩内容是切后台时回读到的离开画面（RGBA8 像素），与原生
    // warmup 呈现的那一帧同源，故撤下时画面不跳变。

    /**
     * 遮罩位图与其来源：来源换了才重建位图（像素拷贝不便宜）。
     *
     * 源是运行期发布的那一帧对象，身份变了才需要重新拷贝像素。
     */
    private var holeCoverBitmap: Bitmap? = null
    private var holeCoverSource: Bridge.CoverFrame? = null

    /**
     * 遮罩的绘制画笔：开滤波，缩放时不出现马赛克。
     *
     * 遮罩帧与开孔不同尺寸时要缩放（输入法改高度），最近邻缩放会把字画成方块。
     */
    private val coverPaint = Paint().apply { isFilterBitmap = true }

    /**
     * 遮罩的绘制矩形：预先分配并复用，避免在绘制路径上每帧新建对象。
     */
    private val coverSourceRect = Rect()
    private val coverDestRect = RectF()

    /**
     * 原生 surface 是否已有缓冲（组合层推来的，仅用于触发重绘）。
     *
     * 画不画遮罩以**运行期的事实**为准（见 [onDraw]），这个字段只负责在事实翻转时
     * 请求重绘——组合层重组的时机不确定，而遮罩的撤下必须及时。
     */
    internal var surfaceHasBuffer: Boolean = true
        set(value) {
            if (field == value) return
            field = value
            invalidate()
        }

    /**
     * 没有可复现的上一帧时铺的底色（终端底色）。
     *
     * 没有它，开孔会透出后方桌面——那正是「闪一下」的另一种样子。
     */
    internal var holeCoverBackground: Int = Color.BLACK
        set(value) {
            if (field == value) return
            field = value
            invalidate()
        }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val runtime = viewModel?.runtime ?: return
        // 原生 surface 已有缓冲：MUST NOT 画遮罩（它不透明，画上去就把终端内容整个
        // 盖住）。判据是运行期的事实（谁呈现过帧），不是本视图的猜测。
        if (runtime.surfaceBufferPresent.value) {
            // 位图也随之释放：此后的每一帧都从这里早退，缓存的那份会一直留到视图销毁。
            releaseCoverBitmap()
            return
        }
        val cover = runtime.coverFrame.value
        // 遮罩帧与当前开孔的尺寸可以不同：输入法弹出/收起会改开孔高度，而遮罩帧
        // 是离开画面时的旧尺寸。按宽度等比缩放、顶边对齐地贴上去（见 `coverFit`），
        // 高出的裁掉、差下的由终端底色补——尺寸必须一致才画的话，输入法收起那一刻
        // 遮罩整块作废，用户看到的就是「输入法弹出时切后台返回终端闪烁」。
        val fit = cover?.let { coverFit(it.width, it.height, width, height) }
        if (cover != null && fit != null) {
            val bitmap = coverBitmap(cover)
            canvas.drawColor(holeCoverBackground)
            coverSourceRect.set(0, 0, bitmap.width, fit.sourceHeight.roundToInt())
            coverDestRect.set(0f, 0f, width.toFloat(), fit.destHeight)
            canvas.drawBitmap(bitmap, coverSourceRect, coverDestRect, coverPaint)
            return
        }
        // 没有可用遮罩（或遮罩已撤下）：铺终端底色。顺带释放缓存的位图——遮罩撤下后
        // 它再无用处，留着白占几百 KB 直到视图销毁。
        releaseCoverBitmap()
        canvas.drawColor(holeCoverBackground)
    }

    /** 释放缓存的遮罩位图与它的来源。 */
    private fun releaseCoverBitmap() {
        holeCoverBitmap = null
        holeCoverSource = null
    }

    /**
     * 取（必要时重建）遮罩位图。
     *
     * 位图与来源一一对应：来源是运行期发布的那一帧对象，身份变了才重建。
     */
    private fun coverBitmap(cover: Bridge.CoverFrame): Bitmap {
        val cached = holeCoverBitmap
        if (cached != null && holeCoverSource === cover) return cached
        val bitmap = cover.toBitmap()
        holeCoverSource = cover
        holeCoverBitmap = bitmap
        return bitmap
    }

    /**
     * 把回读到的遮罩像素转成位图。
     *
     * 像素是 RGBA8，`ARGB_8888` 位图在内存里就是 R、G、B、A 的字节序，
     * 故一次原始拷贝即可，不必逐像素换通道。
     */
    private fun Bridge.CoverFrame.toBitmap(): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.copyPixelsFromBuffer(ByteBuffer.wrap(pixels))
        return bitmap
    }

    // ══════════════════════════════════════════════════════════════════════
    // 五、Surface 生命周期（网格/尺寸由 ResizeManager 拥有）
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 平台重绘请求的同步入口（`SurfaceHolder.Callback2`）。
     *
     * 真正的完成信号必须走 [surfaceRedrawNeededAsync]：Android 13 的
     * `SurfaceView.handleSyncNoBuffer` 先调用重绘回调、再把 syncId 记进 `mSyncIds`。
     * 若完成回调同步执行，syncId 会在被记入之前就随同步组完成而移除，此后每次
     * `surfaceSyncStarted` 都会拿这个陈旧 id 去查同步组，logcat 里便是
     * `SurfaceSyncer: Failed to find sync for id=N`（序号随表面重建递增）。
     * 平台在 Android 14 重写了该路径并删掉了 `mSyncIds`，故只有 13 会命中。
     *
     * 原生渲染器本身异步出帧，这里只负责把「重绘已安排」告诉平台；同步版本不得
     * 自行完成回调，否则等价于旧行为。
     */
    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        viewModel?.runtime?.forceRender()
    }

    /**
     * 重绘完成的异步回调：投递到下一轮 UI 循环再完成，确保平台先完成
     * `mSyncIds` 记账（见 [surfaceRedrawNeeded]），并顺带请求一帧。
     */
    override fun surfaceRedrawNeededAsync(holder: SurfaceHolder, drawingFinished: Runnable) {
        viewModel?.runtime?.forceRender()
        post(drawingFinished)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        // 先认领归属：此后本视图才是唯一有权暂停渲染的那个，旧视图迟到的
        // `surfaceDestroyed` 据此被识别为陈旧（见 `TerminalRuntime.onSurfaceDestroyed`）。
        //
        // MUST 在任何提前返回之前认领：认领被跳过时，旧视图迟到的销毁仍被当作当前
        // 持有者处理——它会清空待绑定的 Surface 并领走一次渲染暂停，而新窗口的绑定
        // 已经发生（此后再无 attach 可归还那次暂停）：终端停在空白画面，渲染线程
        // 无错、无重启、无自愈。
        viewModel?.runtime?.onSurfaceClaimed(surfaceOwnershipToken)
        // 0 尺寸守卫：布局可能在视图测量完成前就调用 surfaceCreated
        // （宽高为 0）——绑定 0×0 的 ANativeWindow 会创建一个
        // acquireNextImage 失败的黑色交换链（一闪而过）。推迟到 surfaceChanged 拿到真实尺寸。
        if (width <= 0 || height <= 0) return
        val surface = holder.surface
        if (!surface.isValid) return
        surfaceWidthPixels = width
        surfaceHeightPixels = height
        viewModel?.let { terminalViewModel ->
            terminalViewModel.surfaceWidth = width
            terminalViewModel.surfaceHeight = height
            terminalViewModel.currentSurface = surface
            val isRunning = terminalViewModel.runtime.state.value.isRunning
            if (!isRunning) {
                terminalViewModel.startRuntime(surface, width, height)
            } else {
                // 重新绑定（重建后的）Surface；渲染器据此重建其 wgpu surface。
                terminalViewModel.runtime.attachSurface(surface, width, height)
                terminalViewModel.runtime.recomputeGrid()
                // onSurfaceDestroyed 在此路径上领了一次暂停（渲染标志置位），
                // 故普通的后台/恢复循环会让该标志保持置位，使重启的渲染线程
                // 输出黑帧（暂停时 render_frame 会短路）。线程重启前清掉它。
                if (!terminalViewModel.runtime.isBackgroundPaused) {
                    forceResumeRendering()
                    terminalViewModel.runtime.resumeRendering()
                    terminalViewModel.runtime.forceRender()
                }
                val runtimeState = terminalViewModel.runtime.state.value
                if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                    rows = runtimeState.rows
                    cols = runtimeState.cols
                }
                // attachSurface 之后 pendingSurface 尺寸才权威：必须在此重算网格，
                // 否则冷启动（会话由 ensureDefaultSession 先建）与后台返回时网格
                // 停在上次 spawn 的默认 24×80，屏幕下半部空白。
                resizeManager.applyGridResize(width, height)
                resizeManager.markConfiguredSize(width, height)
            }
        }
        // 重放 Surface 生效前被丢弃的尺寸：否则冷启动网格停在上次 spawn 的默认 24×80
        // （下半屏空白），直到旋转等外部事件才恢复。
        resizeManager.applyPendingSurfaceResize()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (width <= 0 || height <= 0) return
        surfaceWidthPixels = width
        surfaceHeightPixels = height
        resizeManager.recomputeRowsColsImmediate(width, height)
        // Surface 尺寸变化时总是 resize 交换链，包括输入法的显示/隐藏（仅高度变化）。
        // 旧的「跳过纯高度变化」做法会导致文字拉伸/压缩，
        // 因为视图已 resize 而 GPU 仍在旧尺寸缓冲中渲染，产生非均匀缩放。
        resizeManager.applySurfaceResize(width, height)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        // 只有当前持有者的销毁才允许拆 GPU 面：陈旧视图（Activity 重建时新旧短暂
        // 共存）的这条回调可能晚于新视图的 surfaceCreated 到达，无条件暂停会停掉
        // 新视图刚恢复的渲染线程，且其凭证永不归还 → 永久黑屏。
        val owned = viewModel?.runtime?.onSurfaceDestroyed(surfaceOwnershipToken) == true
        if (owned) {
            viewModel?.runtime?.releaseAllGpuSurfaces()
        }
        // 关闭选区/光标手柄弹窗：它们持有 Activity 上下文，
        // 而「已 dismiss 但仍显示」的弹窗会触发 StrictMode 的
        // Activity 泄漏（在 Activity 销毁期间旋转时还会触发 BadTokenException 崩溃）。
        selectionHandles.hideSelectionHandles()
        // 重置因 Surface 拆除而中断的拖动所残留的任何锁定：
        // 覆盖层已消失故不会再有 UP 到达；陈旧的锁定只能靠下一次触摸自愈。
        handleDragController.reset()
        resizeManager.resetConfiguredSize()
        // 仅在渲染线程被 join 之后才释放 Android Surface。
        //
        // `releaseAllGpuSurfaces()` → `pauseRendering()` 只是把 join **排入**
        // Surface 转换执行器并立即返回，所以此处的 `currentSurface = null`
        // （它决定 `createSession` 还能不能拿到 Surface）仍与渲染线程的 join 并发。
        // 真正的释放必须在 join 之后——这正是 `runAfterRenderThreadsStopped`
        // 的用途：它排在同一个执行器上，因而排在 pauseRendering 的 join 之后。
        //
        // 此前这里在主线程同步清空：注释声称「执行器的顺序保证 join 已完成」，
        // 但该顺序只覆盖 `pauseRendering` 内部，不覆盖这一行。
        if (owned) {
            val runtime = viewModel?.runtime
            val destroyedSurface = holder.surface
            runtime?.runAfterRenderThreadsStopped {
                // 期间新 Surface 可能已绑定（旋转 + 慢渲染线程时是确定性时序）：
                // 无条件清空会让 createSession/switchSession 一直看到 null 而静默
                // 返回。只清掉本次销毁的那一张。
                if (viewModel?.currentSurface === destroyedSurface) {
                    viewModel?.currentSurface = null
                }
            }
        }
    }
}

/** 启动键盘等待焦点的重试节奏与上限。 */
private const val STARTUP_KEYBOARD_RETRY_INTERVAL_MS = 100L
private const val STARTUP_KEYBOARD_ATTEMPTS = 20
