package terminal.emulator.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Matrix
import android.graphics.Rect
import android.graphics.drawable.Drawable
import android.os.SystemClock
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PointerIcon
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.SurroundingText
import android.view.inputmethod.TextAttribute
import android.widget.Magnifier
import android.widget.PopupWindow
import androidx.core.net.toUri
import androidx.core.view.HapticFeedbackConstantsCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.SELECTION_BOUNDS_LENGTH
import terminal.emulator.TerminalViewModel
import terminal.emulator.bridge.Bridge
import terminal.emulator.input.BatchEditDepth
import terminal.emulator.input.ComposingBaseline
import terminal.emulator.input.CursorUpdateMonitor
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.ModifierState
import terminal.emulator.input.SelectionMover
import terminal.emulator.input.applyTerminalEditorInfo
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.DEFAULT_GRID_COLS
import terminal.emulator.runtime.DEFAULT_GRID_ROWS
import terminal.emulator.runtime.InputBatchBuffer
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.computeContentBottomPx
import terminal.emulator.runtime.computeGridAvailableHeight
import terminal.emulator.runtime.computeGridDimensions
import terminal.emulator.runtime.computeImeSurfaceShift
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.util.runCatchingCancellable
import kotlin.math.exp
import kotlin.math.roundToInt

class TerminalSurface
@JvmOverloads
constructor(context: Context, attrs: AttributeSet? = null, defStyleAttr: Int = 0) :
    SurfaceView(context, attrs, defStyleAttr),
    SurfaceHolder.Callback {

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
            val imm =
                context.getSystemService(
                    android.content.Context.INPUT_METHOD_SERVICE,
                ) as android.view.inputmethod.InputMethodManager
            imm.hideSoftInputFromWindow(windowToken, 0)
        } catch (_: Exception) {
            // 视图已拆除——忽略。
        }
        // 清除 fling 滚动结束标记与渲染恢复回调：两者都是捕获本视图的 postDelayed
        // runnable；detach 之后它们会在已销毁的窗口上触发（且重复 fling 会叠加多份）。
        // fling 动画经 postOnAnimation 按 vsync 驱动；在 detach 时显式取消，
        // 使没有任何一步在已销毁的窗口上触发。
        stopFlingAnimation()
        pendingUnpauseRunnable?.let { removeCallbacks(it) }
        pendingUnpauseRunnable = null
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
        pendingImeGridResize?.let { removeCallbacks(it) }
        pendingImeGridResize = null
        // 同理必须停掉平移量的订阅源：它们跑在 viewModelScope 上（跟着宿主而非视图），
        // 保留会让旧视图被协程钉住，且 detach 后的备用屏翻转仍会命中
        // scheduleImeGridResize —— 此时 View.postDelayed 落进 mRunQueue，只有重新
        // attach 才被 drain，被丢弃的旧视图永不 attach，于是配对的 setRenderPaused(false)
        // 永不执行，共享渲染器被永久暂停（终端全黑）。与 pendingSurfaceResize 同类。
        imeShiftJob?.cancel()
        imeShiftJob = null
        // 位移量与扣减量归零：新视图的首次 insets 派发之前不得保留旧值，
        // 否则换视图 / Activity 重建后终端会被上一份键盘高度上移（旋转不重建 Activity，
        // 由 `onSizeChanged` → `applyImeShift` 按新视口高度覆盖，不走这条路径）。
        appliedImeShiftPx = 0
        imeShiftCapacityPx = 0
        imeShiftRevealPx = 0f
        if (translationY != 0f) translationY = 0f
        // 运行期那份也必须归零：它只由本视图的 insets 派发写入，而新视图（或复用后
        // 的本视图）的首次派发若发生在键盘已收起时，算出的 `reserved` 就是 0、与它
        // 自身的 `imeInsetPx` 相等 → 被 `!=` 门控挡下、不再发布，运行期就永久保留
        // 旧的键盘高度（键栏错位、备用屏网格塌缩且不自愈）。
        //
        // 归零 MUST 核对发布者而不是数值：`key(surfaceKey)` 换视图时新旧 detach/attach
        // 的先后没有保证，而同一窗口里两个视图看到的输入法高度相同、按值判断会误判成
        // 「我还是最后发布者」，把新视图刚发布的真实高度覆盖成 0 且此后永不修复。
        imeInsetPx = 0
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
        try {
            magnifier?.dismiss()
        } catch (exception: Exception) {
            LogUtil.w(TAG, "onDetachedFromWindow: magnifier dismiss failed", exception)
        }
        magnifier = null
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
        val bridge = ptyWriteQueueBridge
        if (bridge != null && viewModel?.runtime?.bridge() === bridge) {
            bridge.ptyWriteQueue = null
        }
        ptyWriteQueueBridge = null
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
        installPtyWriteQueue()
        // 订阅必须随 attach 恢复，不能只在 attachViewModel 里建一次：detach 会停掉它们
        // （见 onDetachedFromWindow 的清理），而视图复用时不会重新走 AndroidView.factory，
        // 于是 altScreenActiveFlow / lastContentRowFlow / cellMetricsFlow 三条输入全部失联
        // ——网格再也不重排、主屏位移不再跟随内容。
        viewModel?.let { observeImeShiftInputs(it) }
    }

    fun setDimensions(rows: Int, cols: Int) = resizeManager.setDimensions(rows, cols)

    /** 直接转发：选区手柄弹窗由 [SelectionHandles] 实现。 */
    fun showSelectionHandles(startRow: Int, startCol: Int, endRow: Int, endCol: Int, themeFgColor: Int) =
        selectionHandles.showSelectionHandles(startRow, startCol, endRow, endCol, themeFgColor)

    /** 直接转发：选区手柄弹窗由 [SelectionHandles] 实现。 */
    fun hideSelectionHandles() = selectionHandles.hideSelectionHandles()

    private var selectionMenuPopup: PopupWindow? = null

    /** 菜单条视图，与 [selectionMenuPopup] 同生命周期（原地换条目时复用，见 [showSelectionMenu]）。 */
    private var selectionMenuBar: android.widget.LinearLayout? = null

    /**
     * 菜单窗口里已建好的条目文案。条目集不变就什么都不动；变了（例如剪贴板由空变
     * 非空、PASTE 因此出现）也只原地换条目——窗口本身一次选区会话只创建一次。
     *
     * 复用前必须确认窗口还活着：平台直接移除弹窗（父视图窗口令牌变化）不会走
     * [hideSelectionMenu]，此时 `isShowing` 仍为 true，只看它就会把菜单永久留在
     * 一个已消失的窗口上（`contentView.isAttachedToWindow` 是可靠的存活判据）。
     */
    private var shownMenuLabels: List<String>? = null

    /**
     * 选区上下文菜单，以 [PopupWindow] 实现（与选区手柄相同的独立系统窗口模式）：
     * 旧的 Compose 菜单会被 SurfaceView 的开孔遮住，而 ActionMode TYPE_FLOATING 工具栏
     * 在 API 35 模拟器上不渲染（模拟器已验证：手柄可见、工具栏缺失）。
     * PopupWindow 在所有平台上都渲染在 SurfaceView 之上。
     *
     * 菜单项对标 termux：文本选区提供 COPY | SELECT ALL；仅粘贴的（空白单元格）
     * 选区提供 PASTE——且仅当剪贴板确实有文本时（ClipboardAccess.hasClipboardText），
     * 使失效的 PASTE 动作永不出现（即「PASTE 按钮始终显示」这一反馈）。
     * 关闭沿用既有的选区状态流：点击外部会抵达终端并清除选区，
     * TerminalScreen 的 LaunchedEffect 随即调用 [hideSelectionMenu]。
     *
     * **窗口一次选区会话只创建一次**：此后状态怎么变都原地更新（[PopupWindow.update]
     * 挪锚点、[fillMenuBar] 换条目）。此前每次 show 都 dismiss + 重建，旧窗口已从窗口
     * 管理器消失、新窗口还没挂上，中间几百毫秒里触摸落到下层——铺满整个表面的手柄
     * 覆盖层，再转发给终端并清掉选区。用户看到的就是「所有菜单按钮无效、点一下菜单
     * 自己消失」（实测：点击前 0.5s 有一次 showSelectionMenu 触发的重建）。
     */
    fun showSelectionMenu(pasteOnly: Boolean) {
        // 覆盖层不因菜单显示而收起：它只在柄位矩形内消费触摸、其余转发给表面，
        // 抓柄时 latchDragAnchor 收起菜单、抬手由 finishHandleDrag 重锚重显——
        // 若顺手收起覆盖层，那条路径就成了死代码，长按选词之后再也没法拖柄扩选。
        // 两者叠放不靠创建先后：覆盖层用 TYPE_APPLICATION_PANEL、菜单用
        // TYPE_APPLICATION_SUB_PANEL，层号高者恒在上，覆盖层重建也压不住菜单。
        val selection = viewModel?.state?.value?.selection
        if (selection == null) {
            LogUtil.w(TAG, "菜单跳过：视图模型为空")
            return
        }
        if (selection.start == null || selection.end == null) {
            LogUtil.w(TAG, "菜单跳过：选区无界")
            return
        }
        // 文本区动作的显示判定与动作解析共用同一份文本（viewModel.selectionTextForAction
        // → SelectionManager.resolvedSelectionText）。空文本的选区不再挂出「复制/分享」：
        // 点下去只会静默返回，用户看到的是「按钮无效」，而这与「按不动」无法区分。
        val hasCopyableText = viewModel?.selectionTextForAction()?.isNotEmpty() == true
        val actions =
            menuActionsForSelection(pasteOnly, clipboardAccess.hasClipboardText(), hasCopyableText)
        if (actions.isEmpty()) {
            LogUtil.d(TAG, "菜单跳过：无动作")
            return
        }
        val anchor = menuAnchor(selection)
        if (anchor == null) {
            LogUtil.d(TAG, "菜单跳过：无处安放")
            return
        }
        val loc = IntArray(2)
        getLocationInWindow(loc)
        val existing = selectionMenuPopup
        val existingBar = selectionMenuBar
        if (existing != null &&
            existingBar != null &&
            existing.isShowing &&
            existing.contentView.isAttachedToWindow
        ) {
            val labels = actions.map { it.first }
            if (labels != shownMenuLabels) fillMenuBar(existingBar, actions)
            shownMenuLabels = labels
            existing.update(
                loc[0] + anchor.first,
                loc[1] + anchor.second,
                android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
                android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
            )
            LogUtil.d(
                TAG,
                "菜单原地更新：at=(${loc[0] + anchor.first},${loc[1] + anchor.second}) labels=$labels",
            )
            return
        }
        // 旧窗口已不在（平台移除或已 dismiss）：清掉引用再重建。
        hideSelectionMenu("showSelectionMenu", restoreHandles = false)
        if (viewDetached || !isAttachedToWindow) {
            LogUtil.w(TAG, "菜单跳过：未附着")
            return
        }
        val bar = buildMenuBar(actions)
        val popup =
            PopupWindow(
                bar,
                android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
                android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
            )
                .apply {
                    isOutsideTouchable = false
                    isFocusable = false
                    setBackgroundDrawable(null)
                    setAnimationStyle(0)
                    setWindowLayoutType(
                        android.view.WindowManager.LayoutParams.TYPE_APPLICATION_SUB_PANEL,
                    )
                }
        try {
            // 本回调只在**我们**调用 dismiss() 时触发。平台直接移除弹窗窗口
            // （父视图窗口令牌变化/销毁）不会走到这里——两者是否成对出现，
            // 是区分「应用主动关菜单」与「平台把窗口收走」的唯一依据。
            popup.setOnDismissListener { LogUtil.d(TAG, "选区菜单 dismiss() 已执行（应用侧主动关闭）") }
            popup.showAtLocation(this@TerminalSurface, 0, loc[0] + anchor.first, loc[1] + anchor.second)
            LogUtil.d(
                TAG,
                "菜单显示：at=(${loc[0] + anchor.first},${loc[1] + anchor.second}) " +
                    "pasteOnly=$pasteOnly actions=${actions.size}",
            )
        } catch (exception: Exception) {
            // Activity 在检查与显示之间被 detach——与选区手柄同一守卫；
            // 弹窗从未变为可见。
            LogUtil.w(TAG, "showSelectionMenu: popup show failed", exception)
            return
        }
        selectionMenuPopup = popup
        selectionMenuBar = bar
        shownMenuLabels = actions.map { it.first }
    }

    /** 选区可用菜单项：对标 termux 语义（COPY | SHARE | SELECT ALL | OPEN LINK，剪贴板有文本时提供 PASTE）。
     *
     * [hasCopyableText] 由调用方用 [TerminalViewModel.selectionTextForAction]（即动作真正
     * 复制的那份文本）求得：为空时 COPY/SHARE 不出现——它们是唯一的「点了没反应」入口。
     */
    internal fun menuActionsForSelection(
        pasteOnly: Boolean,
        pasteEnabled: Boolean,
        hasCopyableText: Boolean = true,
    ): List<Pair<String, () -> Unit>> = buildList {
        if (pasteOnly) {
            if (pasteEnabled) {
                add(
                    context.getString(R.string.paste) to
                        {
                            viewModel?.pasteFromClipboard()
                            viewModel?.clearSelection()
                        },
                )
            }
        } else {
            if (hasCopyableText) {
                add(
                    context.getString(R.string.copy) to
                        {
                            viewModel?.copySelectionToClipboard()
                            viewModel?.clearSelection()
                        },
                )
                add(
                    context.getString(R.string.share) to
                        {
                            viewModel?.shareSelection()
                            viewModel?.clearSelection()
                        },
                )
            }
            add(
                context.getString(R.string.select_all) to
                    {
                        // termux 行为：全选后保持菜单打开，用户可立即复制新选区。
                        viewModel?.selectAll()
                        // 选择几何剧变：隐藏→按新界限重显完成重锚（design 决策 3）。
                        showSelectionMenuForCurrentSelection()
                    },
            )
            // 打开链接项：仅 OSC 8 超链接（libghostty-vt 不识别纯文本裸 URL）。
            // 显示判定与动作解析共用同一输入，绝不出现“项显示却无目标”。
            val selectionHyperlinkUri = selectionHyperlinkUri()
            if (selectionHyperlinkUri != null) {
                add(
                    context.getString(R.string.open_link) to
                        {
                            openSelectionAsLink(selectionHyperlinkUri)
                            viewModel?.clearSelection()
                        },
                )
            }
        }
    }

    /** 选区起点的 OSC 8 超链接 URI，无链接返回 null。 */
    private fun selectionHyperlinkUri(): String? = viewModel
        ?.state
        ?.value
        ?.selection
        ?.start
        ?.let { viewModel?.runtime?.bridge()?.hyperlinkAt(it.row, it.col) }
        ?.let { resolveOpenLinkUri(it) }

    /** 菜单“打开链接”动作：http(s) 白名单后交系统打开；无目标静默返回。 */
    internal fun openSelectionAsLink(hyperlinkUri: String) {
        val target = resolveOpenLinkUri(hyperlinkUri) ?: return
        val uri = try {
            target.toUri()
        } catch (_: IllegalArgumentException) {
            return
        }
        val scheme = uri.scheme?.lowercase(java.util.Locale.ROOT)
        if (scheme != "http" && scheme != "https") return
        try {
            val intent =
                android.content.Intent(android.content.Intent.ACTION_VIEW, uri)
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
            context.startActivity(intent)
        } catch (exception: Exception) {
            LogUtil.w(TAG, "openSelectionAsLink: no handler", exception)
        }
    }

    /**
     * 深色胶囊工具栏，每个动作用一个可点击标签承载（创建窗口时用；
     * 已在屏幕上的菜单原地换条目走 [fillMenuBar]）。
     */
    private fun buildMenuBar(actions: List<Pair<String, () -> Unit>>): android.widget.LinearLayout {
        val bar = android.widget.LinearLayout(context)
        fillMenuBar(bar, actions)
        return bar
    }

    /** 把 [actions] 装进 [bar]：先清空再重建按钮，窗口与背景复用（见 [showSelectionMenu]）。 */
    private fun fillMenuBar(bar: android.widget.LinearLayout, actions: List<Pair<String, () -> Unit>>) {
        val density = resources.displayMetrics.density
        fun densityPixels(value: Int): Int = (value * density + HALF_PIXEL_OFFSET).toInt()
        bar.orientation = android.widget.LinearLayout.HORIZONTAL
        val backgroundDrawable = android.graphics.drawable.GradientDrawable()
        backgroundDrawable.setColor(resolveThemeColor(R.attr.colorSurface, R.color.material_color_surface))
        backgroundDrawable.cornerRadius = densityPixels(MENU_BAR_CORNER_RADIUS_DP).toFloat()
        bar.background = backgroundDrawable
        bar.elevation = densityPixels(MENU_ELEVATION_DP).toFloat()
        bar.removeAllViews()
        for ((label, action) in actions) {
            val item =
                android.widget.TextView(context).apply {
                    text = label
                    setTextColor(resolveThemeColor(R.attr.colorOnSurface, R.color.material_color_on_surface))
                    textSize = MENU_ITEM_TEXT_SIZE_SP
                    val horizontalPadding = densityPixels(MENU_ITEM_HORIZONTAL_PADDING_DP)
                    setPadding(
                        horizontalPadding,
                        densityPixels(MENU_ITEM_VERTICAL_PADDING_DP),
                        horizontalPadding,
                        densityPixels(MENU_ITEM_VERTICAL_PADDING_DP),
                    )
                    isClickable = true
                    isFocusable = false
                    setOnClickListener { action() }
                }
            bar.addView(item)
        }
    }

    /**
     * 解析 Material 3 主题颜色属性（spec：选择菜单样式 MUST 用 `colorSurface`/
     * `colorOnSurface` 主题属性、MUST NOT 硬编码颜色）；属性未定义时回退同值的
     * 颜色资源，Kotlin 侧不出现任何字面颜色值。
     */
    private fun resolveThemeColor(attribute: Int, fallbackColorResource: Int): Int {
        val typedValue = android.util.TypedValue()
        if (!context.theme.resolveAttribute(attribute, typedValue, true)) {
            return androidx.core.content.ContextCompat.getColor(context, fallbackColorResource)
        }
        return if (typedValue.resourceId != 0) {
            androidx.core.content.ContextCompat.getColor(context, typedValue.resourceId)
        } else {
            typedValue.data
        }
    }

    /**
     * 选择几何（网格 → 视图像素）→ 纯函数 [menuAnchor] 的输入：选择矩形、视口
     * 与菜单估计尺寸。返回 null（无处可放/度量未就绪）时调用方隐藏菜单。
     */
    private fun menuAnchor(selection: terminal.emulator.SelectionState): Pair<Int, Int>? {
        val cellWidthPixels = cellWidth
        val cellHeightPixels = cellHeight
        if (cellWidthPixels <= 0f || cellHeightPixels <= 0f) return null
        val start = selection.start ?: return null
        val end = selection.end ?: return null
        // 行按先后归一（与取文共用 `normalizeSelectionAnchors`）；列取包围盒极值——
        // 多行选区的列是各行内偏移，不存在「全局左右」，故不能跟着行一起换。
        val (lo, hi) = normalizeSelectionAnchors(start, end, { it.row }, { it.col })
        val topRow = lo.row
        val bottomRow = hi.row
        val leftCol = minOf(start.col, end.col)
        val rightCol = maxOf(start.col, end.col)
        val viewportTopGrid = currentViewportTopGrid()
        val (leftPx, topPx) = gridToScreen(topRow, leftCol, viewportTopGrid, cellWidthPixels, cellHeightPixels)
        val (rightPx, bottomPx) =
            gridToScreen(bottomRow + 1, rightCol + 1, viewportTopGrid, cellWidthPixels, cellHeightPixels)
        val density = resources.displayMetrics.density
        val densityPixels = { value: Int -> (value * density + HALF_PIXEL_OFFSET).toInt() }
        // PopupWindow 在显示时才测量，故用粗略估算（项数 × ~92dp）并钳位到 Surface。
        val estimatedWidth = width.coerceAtMost(densityPixels(MENU_ESTIMATED_WIDTH_DP))
        val menuHeight = densityPixels(MENU_HEIGHT_DP)
        return menuAnchor(
            selection = PixelRect(leftPx.toInt(), topPx.toInt(), rightPx.toInt(), bottomPx.toInt()),
            viewport = PixelRect(0, 0, width, height),
            menuWidth = estimatedWidth,
            menuHeight = menuHeight,
            handleHeight = selectionHandleHeight().toInt(),
        )
    }

    /** 选择手柄高度（与手柄定位同源；未知时回退一 Character 行高）。 */
    private fun selectionHandleHeight(): Float {
        val content = selectionHandles.contentHandleHeight()
        return if (content > 0) content.toFloat() else cellHeight
    }

    /**
     * 关闭选区菜单弹窗。
     *
     * [reason] 必填：关闭完全由外部驱动（detach／抓柄／IME 切换／非手柄轻击／
     * 状态流重锚），无此参数时菜单意外消失只能靠猜。
     *
     * [restoreHandles] 为 false 时不动手柄覆盖层——只给「关掉是为了马上重开菜单」的
     * 调用点用（见 [showSelectionMenu]），否则关闭菜单后按当前选区恢复手柄。
     */
    fun hideSelectionMenu(reason: String, restoreHandles: Boolean = true) {
        val popup = selectionMenuPopup
        selectionMenuPopup = null
        selectionMenuBar = null
        shownMenuLabels = null
        if (popup == null) return
        // 记下消失瞬间的选区状态：菜单的关闭完全由选区状态流驱动，
        // 而 dismiss 的调用点有四条（detach／抓柄／IME 切换／非手柄轻击），
        // 无此日志时菜单意外消失只能靠猜——PasteButtonInstrumentedTest 在
        // CI 同款几何下曾整段丢失粘贴动作而日志里查不到是谁关的。
        val selection = viewModel?.state?.value?.selection
        LogUtil.d(
            TAG,
            "hideSelectionMenu($reason): pasteOnly=${selection?.pasteOnly} " +
                "menuDismissed=${selection?.menuDismissed}",
        )
        try {
            popup.dismiss()
        } catch (exception: Exception) {
            // 已 dismiss——只记日志，使原因不丢失。
            LogUtil.w(TAG, "hideSelectionMenu: dismiss failed", exception)
        }
        if (restoreHandles) reshowSelectionHandles()
    }

    // ══════════════════════════════════════════════════════════════════════
    // 五之二、尺寸/网格计算
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有网格/尺寸计算：视图尺寸 → 行列 → PTY resize 与交换链重配置。
     * 内部类：直接访问外层视图的 rows/cols/lastConfigured* 字段。
     */
    inner class ResizeManager {
        /**
         * 单一网格公式，与 `recomputeGridFromFontMetrics` 共用：
         * rows = (surface − ModifierBar − 备用屏输入法遮挡) / cell，cols = surface / cell。
         *
         * 输入法遮挡**只在备用屏扣除**（见 [imeInsetPx]）：主屏靠纯平移跟随键盘
         *（TESTING.md 要求上移后底部像素与上移前完全相同），其显示/隐藏绝不能改变
         * rows/cols，否则会有重排闪烁、换行错乱、底部行丢失。
         */
        internal fun applyGridResize(width: Int, height: Int) {
            val runtime = viewModel?.runtime ?: return
            val cellWidth = runtime.cellWidth
            val cellHeight = runtime.cellHeight
            if (cellWidth <= 0f || cellHeight <= 0f) return
            // 备用屏按可见高度重排（SIGWINCH），主屏恒为 0。取运行期的单一值：
            // 三条网格路径（此处、`recomputeRowsColsImmediate`、运行期的
            // `recomputeGridFromFontMetrics`）必须扣同一个数，否则备用屏会在字号变化
            // 后被撑回被键盘遮住的高度且不会自愈。
            val imeReserve = runtime.imeGridReserve()
            // 高度是 SurfaceView 的布局高度。ModifierBar 覆盖其底部，
            // 故计算 rows 之前减去其高度——与运行期施加的预留量相同。
            val availableHeight =
                computeGridAvailableHeight(
                    surfaceHeight = height,
                    modifierBarHeightPx = runtime.modifierBarHeightPx,
                    imeReserve = imeReserve,
                )
            val (newRows, newCols) =
                computeGridDimensions(
                    surfaceWidth = width,
                    surfaceHeight = availableHeight,
                    cellWidth = cellWidth,
                    cellHeight = cellHeight,
                )
            if (newRows == 0 || newCols == 0) return
            LogUtil.d(
                "TerminalSurface",
                "applyGridResize: $width x $height cell=($cellWidth,$cellHeight) " +
                    "-> ${newRows}x$newCols (was ${rows}x$cols)",
            )
            if (newRows != rows || newCols != cols) {
                runtime.resize(newRows, newCols)
                rows = newRows
                cols = newCols
            }
            // 推入像素尺寸与网格变化解耦：输入法遮挡变化后行数可能恰好不变
            // （矮键盘 + 大单元格），但 ws_ypixel 描述的网格区域已经变了。
            // 契约见 NativeBridge.setPixelSize：每次网格 resize 都随 surface 的
            // 像素尺寸一并下发。availableHeight 已排除键栏与输入法遮挡——
            // 正是 rows 覆盖的网格区域。
            runtime.setPixelSize(width, availableHeight)
        }

        internal fun recomputeRowsColsImmediate(width: Int, height: Int) {
            val viewModel = viewModel
            if (viewModel != null) {
                val cellWidth = viewModel.runtime.cellWidth
                val cellHeight = viewModel.runtime.cellHeight
                if (cellWidth > 0f && cellHeight > 0f) {
                    // 与运行期网格相同的预留量：不减去工具栏高度时，
                    // 此镜像会相差工具栏那几行，并在每次重组时被迫 requestLayout()。
                    val barPx = viewModel.runtime.modifierBarHeightPx
                    val availableHeight =
                        computeGridAvailableHeight(
                            surfaceHeight = height,
                            modifierBarHeightPx = barPx,
                            imeReserve = viewModel.runtime.imeGridReserve(),
                        )
                    val (newRows, newCols) =
                        computeGridDimensions(
                            surfaceWidth = width,
                            surfaceHeight = availableHeight,
                            cellWidth = cellWidth,
                            cellHeight = cellHeight,
                        )
                    if (newRows == 0 || newCols == 0) return
                    cols = newCols
                    rows = newRows
                    return
                }
            }
            // 度量尚未就绪（原生字体还没回读）时的兜底：由上次配置的像素尺寸反推
            // 单元格尺寸。同样扣掉键栏与输入法遮挡——`lastConfigured*` 是整块 Surface
            // 尺寸，而 `rows` 是扣减后算出的行数，两者口径必须一致，否则这里算出的
            // 行数会与随后由真度量算出的结果跳变。
            if (lastConfiguredWidth > 0 && lastConfiguredHeight > 0 && rows > 0 && cols > 0) {
                val runtime = viewModel?.runtime
                val cellWidthPx = lastConfiguredWidth.toFloat() / cols
                // 扣减走同一个算式（与上方度量就绪分支逐字一致）：此前此处把
                // 「键栏 + 输入法遮挡」手工相加且没有下限钳位，只是碰巧在
                // Surface 被完全遮住时与另一条分支算出同一个结果。
                val cellHeightPx =
                    computeGridAvailableHeight(
                        surfaceHeight = lastConfiguredHeight,
                        modifierBarHeightPx = runtime?.modifierBarHeightPx ?: 0,
                        imeReserve = runtime?.imeGridReserve() ?: 0,
                    ).toFloat() / rows
                // 上界：兜底用的 lastConfigured* 严重失配时，cellWidthPx 可能小到
                // 让商溢出 Int（Float.toInt() 饱和成 Int.MAX_VALUE），进而把 PTY 网格
                // 撑到 65535 列。手机上的终端最多几百列，512 足够宽裕。
                cols = (width.toFloat() / cellWidthPx).toInt().coerceIn(1, MAX_FALLBACK_GRID_SIZE)
                rows =
                    (
                        computeGridAvailableHeight(
                            surfaceHeight = height,
                            modifierBarHeightPx = runtime?.modifierBarHeightPx ?: 0,
                            imeReserve = runtime?.imeGridReserve() ?: 0,
                        ).toFloat() / cellHeightPx
                        ).toInt().coerceIn(1, MAX_FALLBACK_GRID_SIZE)
            }
        }

        internal fun applySurfaceResize(width: Int, height: Int) {
            if (width <= 0 || height <= 0) return
            if (
                width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0
            ) {
                return
            }
            // 首次布局：立即应用（避免启动黑屏闪烁）。
            // 后续变化（输入法动画帧、Gboard 候选栏闪烁）：
            // 经 IME_RESIZE_DEBOUNCE_MS 防抖到稳定尺寸
            // ——否则每个中间尺寸都会强制一次完整的交换链重配置
            // 与 PTY 网格重排（掉帧、CellData 竞争错误、耗电）。
            // 防抖期间暂停渲染，使陈旧尺寸的缓冲绝不会被拉伸到正在动画的视图上
            // （显示时压扁、隐藏时拉伸）；下方的稳定触发会恢复并呈现一帧新画面。
            if (lastConfiguredWidth == 0) {
                applySurfaceResizeNow(width, height)
                return
            }
            // 顺序 MUST 是「先领新的、再归还旧的」：两者都持有时 ledger 的持有者数
            // 走 1 → 2 → 1，全程不落到 0，因而不发出任何恢复/暂停回调。反过来
            // （先归还再领）会让它经过 0，每个替换帧都产生一对多余的
            // `setRenderPaused(false)`/`(true)`，与「备用屏重排期间不出帧」冲突，
            // 还多出两次 JNI 往返。
            //
            // 被 `removeCallbacks` 丢弃的 runnable 永不执行，它领到的那次暂停必须在此
            // 归还（见 [RenderPauseLedger.release]）；`onSizeChanged` 与
            // `surfaceChanged` 常在同一帧用同一尺寸各调一次本方法，正是这条替换路径的
            // 常见触发。
            val token = viewModel?.runtime?.acquireRenderPause()
            pendingSurfaceResize?.let { removeCallbacks(it) }
            pendingSurfaceResize = null
            pendingSurfaceResizeToken?.let { viewModel?.runtime?.releaseRenderPause(it) }
            pendingSurfaceResizeToken = token
            pendingSurfaceResize =
                Runnable {
                    pendingSurfaceResize = null
                    // 以最新尺寸为准：onSizeChanged 已存储了它。
                    applySurfaceResizeNow(surfaceWidthPixels, surfaceHeightPixels)
                    // 稳定触发可能恰好落在已配置的尺寸上而提前返回、来不及执行自身的
                    // 恢复——故始终在此归还。
                    //
                    // 归还的是**本 runnable 领到的那一枚**，且要确认它仍是当前在途的
                    // 那一枚：视图 detach 时 `removeCallbacks` 够不到 attach 期间投递的
                    // runnable（`mAttachInfo` 已置空，改投 `mRunQueue`），同一实例重新
                    // attach 后它仍会执行——若归还是无条件的，它会去还掉新防抖刚领的那枚。
                    if (token != null && pendingSurfaceResizeToken == token) {
                        pendingSurfaceResizeToken = null
                        viewModel?.runtime?.releaseRenderPause(token)
                    }
                }
                    .also { postDelayed(it, IME_RESIZE_DEBOUNCE_MS) }
        }

        /**
         * 尺寸在 Surface 生效前到达时的暂存值。`onSizeChanged` 先于
         * `surfaceCreated`/`surfaceChanged` 触发（布局阶段 SurfaceHolder 尚无有效 Surface），
         * [applyResizeNormal] 在该分支直接返回、尺寸被永久丢弃：网格停在上次 spawn 的默认
         * 24×80，屏幕下半部空白，直到下一次外部尺寸事件（旋转）才恢复。
         */
        private var pendingRetryWidth: Int = 0
        private var pendingRetryHeight: Int = 0

        /** Surface 生效后重放被丢弃的尺寸；无暂存值或尺寸未变时为空操作。 */
        internal fun applyPendingSurfaceResize() {
            val width = pendingRetryWidth
            val height = pendingRetryHeight
            if (width <= 0 || height <= 0) return
            pendingRetryWidth = 0
            pendingRetryHeight = 0
            applySurfaceResizeNow(width, height)
        }

        internal fun applySurfaceResizeNow(width: Int, height: Int) {
            if (width <= 0 || height <= 0) return
            // 尺寸振荡后的延迟触发可能又落回已配置的尺寸——跳过冗余的重配置。
            if (
                width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0
            ) {
                return
            }
            val terminalViewModel = viewModel ?: return
            terminalViewModel.surfaceWidth = width
            terminalViewModel.surfaceHeight = height

            // 尺寸经下方的 attachSurface 交给原生；不存在独立的 Surface 尺寸通道。
            applyResizeNormal(width, height, terminalViewModel)
        }

        internal fun applyResizeNormal(width: Int, height: Int, terminalViewModel: TerminalViewModel) {
            terminalViewModel.runtime.recomputeGrid()
            val surface = holder.surface
            if (!surface.isValid) {
                LogUtil.d(TAG, "applySurfaceResize: surface not valid yet, deferring")
                pendingRetryWidth = width
                pendingRetryHeight = height
                return
            }
            terminalViewModel.currentSurface = surface
            // 把 Surface 交给原生；渲染器据此创建 wgpu surface
            // （attachWindow JNI 在 Rust 内部提取 ANativeWindow）。
            terminalViewModel.runtime.attachSurface(surface, width, height)
            val runtimeState = terminalViewModel.runtime.state.value
            if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                rows = runtimeState.rows
                cols = runtimeState.cols
            } else if (!runtimeState.isRunning) {
                // start() 在过小的 Surface 上会提前退出（分屏、自由窗口、
                // 可折叠半屏）且没有任何重试——窗口变大后终端将永远空白，
                // 因为 surfaceChanged 只会 resize。趁 Surface 此刻有效且已定尺寸，
                // 在此重试会话创建。
                LogUtil.i(TAG, "applySurfaceResize: runtime not started, retrying default session")
                terminalViewModel.ensureDefaultSession()
            }
            lastConfiguredWidth = width
            lastConfiguredHeight = height
            // 切后台返回经 surfaceChanged 重建交换链后强制一帧：闲时无新输出也呈现，避免黑屏。
            //
            // 但后台期间 MUST NOT 恢复：ON_PAUSE 收起输入法会触发一次尺寸变化，
            // 那时窗口可能已被系统回收，恢复渲染只会取帧失败（永久黑屏），
            // 还会多出一个与推进泵重复消费全局事件队列的渲染线程。
            if (!terminalViewModel.runtime.isBackgroundPaused) {
                forceResumeRendering()
                terminalViewModel.runtime.resumeRendering()
                terminalViewModel.runtime.forceRender()
            }
            // 旋转/窗口尺寸变化（无输入法事件时）永远不会到达 runtime.resize：
            // 另一个触发点只有 insets 派发回调。
            // 使用共享公式使两条路径对网格的认知一致。
            // 仅在真实单元格度量到达后生效（此前为空操作）。
            // 输入法 inset 只在备用屏影响网格（见 imeGridReserve）：主屏靠纯平移
            // 跟随键盘，改它的 rows/cols 会有重排闪烁与底部行丢失。
            applyGridResize(width, height)
        }

        internal fun setDimensions(rows: Int, cols: Int) {
            this@TerminalSurface.rows = rows
            this@TerminalSurface.cols = cols
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 五之三、输入法 InputConnection
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有输入法 InputConnection：组字跟踪、提交/删除处理
     * 以及 EditorInfo 属性。外层以轻量转发暴露 finishComposing。
     */
    inner class ImeConnection {
        var currentInputConnection: InputConnection? = null

        fun createInputConnection(outAttrs: EditorInfo): InputConnection {
            applyTerminalEditorInfo(outAttrs)
            val connection =
                object : BaseInputConnection(this@TerminalSurface, true) {
                    // 进行中的输入法组字：使增量得以校对而非被丢弃。
                    // 基线下沉为可单测的持有者：取文接口与组字光标必须同源，
                    // 否则输入法读到的与自己写下的不一致。
                    private val composing = ComposingBaseline()

                    // 批量编辑嵌套与选区折算都是与 Android 生命周期无关的纯状态，
                    // 故下沉为可单测类型（ImeCursorContract）。
                    private val batchEdits = BatchEditDepth()
                    private val selectionMover = SelectionMover(maxMagnitude = MAX_SELECTION_MOVE)
                    private val cursorMonitor = CursorUpdateMonitor()

                    private fun encodeAndSend(text: String, ctrlActive: Boolean, altActive: Boolean) {
                        inputBatchBuffer.write(
                            TerminalInputEncoder.encodeCommittedText(
                                text = text,
                                ctrlActive = ctrlActive,
                                altActive = altActive,
                            ),
                        )
                    }

                    // 同一事务的退格与追加走同一出口（N1-27）：退格同步直写而追加经批缓冲
                    // 异步发送时，超量追加被推迟到下一帧，退格先到即顺序反转。
                    private fun sendBackspaces(count: Int) {
                        if (count > 0) inputBatchBuffer.write(ByteArray(count) { BACKSPACE_BYTE })
                    }

                    /**
                     * 受理输入法的光标位置监视请求。
                     *
                     * 返回 true 是因为本仓**确实**会回报（见 [reportComposingCursorAnchor]）；
                     * 返回 false 会让输入法以为锚点永不更新，从此不再请求，候选窗口
                     * 也就永远贴不到真实光标。
                     */
                    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
                        cursorMonitor.update(cursorUpdateMode)
                        reportComposingCursorAnchor(immediate = true)
                        return true
                    }

                    /**
                     * 把组字光标锚点回报给输入法。
                     *
                     * 准入与落格见 [composingCursorMarker]；坐标取最近一帧的终端光标格——
                     * 组字字节正写进那一格。无组字或输入法未请求监视时不回报，
                     * 否则候选窗口会被贴到不属于它的格子上。
                     *
                     * @param immediate 跳过限频。输入法刚改监视模式时要立刻拿到位置，
                     *   等下一次组字变化会让候选窗口先落在旧格上。
                     */
                    private fun reportComposingCursorAnchor(immediate: Boolean = false) {
                        // 准入判据先于取坐标：未请求监视或无组字时连帧内光标都不必取。
                        if (!cursorMonitor.isMonitoring || !composing.isComposing) return
                        // 输入法尚未接管本视图（IME 动画/焦点切换的中间帧）时提交锚点，
                        // 平台会打 "requestCursorAnchorInfo on inactive InputConnection"
                        // 并丢弃这次更新；等真正获得焦点后的组字变化再报。
                        if (!hasWindowFocus() || !isFocused) return
                        if (!immediate) {
                            val now = System.nanoTime()
                            if (now - lastCursorAnchorQueryNanos < CURSOR_ANCHOR_QUERY_THROTTLE_NANOS) return
                        }
                        val runtime = viewModel?.runtime ?: return
                        // 取**最近一帧**的光标：渲染线程已把它发布在会话条目上，这里只做一次
                        // 原子读。旧路每次组字都发一次同步 VT 查询，实测回退一键要等
                        // 18ms(p50)/38ms(p95)，全部落在主线程上。
                        val packed = runtime.bridge()?.renderedCursorPacked() ?: return
                        if (packed < 0) return
                        val cursorRow = (packed shr 32).toInt()
                        val cursorCol = (packed and 0xFFFFFFFFL).toInt()
                        val marker =
                            composingCursorMarker(
                                cursorRow = cursorRow,
                                cursorCol = cursorCol,
                                rows = rows,
                                cellWidth = cellWidth,
                                cellHeight = cellHeight,
                            ) ?: return
                        // 时间戳只在**真的产出了锚点**之后推进（同回滚长度查询）：
                        // `packed < 0` 是取不到光标（首帧未到或光标不可见），标记为 null 则是
                        // 光标落在视口外，两者都不该吃掉限频窗口。
                        lastCursorAnchorQueryNanos = System.nanoTime()
                        val info =
                            CursorAnchorInfo.Builder()
                                // 局部→屏幕的折算矩阵：只给定位参数而不给矩阵时
                                // `CursorAnchorInfo.Builder.build()` 直接抛
                                // （坐标是视图局部的，平台不会替调用方补这一步）。
                                .setMatrix(cursorAnchorMatrix())
                                .setComposingText(0, composing.text)
                                .setInsertionMarkerLocation(
                                    marker.horizontal,
                                    marker.top,
                                    marker.baseline,
                                    marker.bottom,
                                    0,
                                )
                                .build()
                        context.getSystemService(InputMethodManager::class.java)
                            ?.updateCursorAnchorInfo(this@TerminalSurface, info)
                    }

                    /** [getLocationOnScreen] 的落点，复用以免逐键分配。仅主线程使用。 */
                    private val anchorLocation = IntArray(2)

                    /** 局部→屏幕的换算矩阵，复用同上。 */
                    private val anchorMatrix = Matrix()

                    /**
                     * 锚点坐标（视图局部）→ 屏幕坐标的矩阵。
                     *
                     * 本视图不在屏幕原点，且输入法弹出时**自身**就被 `translationY` 上移
                     * （见 [applyImeShift]）——`getLocationOnScreen` 每次都把当前位移算进去，
                     * 故候选窗口跟着光标走，不会停在未上移的旧位置。
                     */
                    private fun cursorAnchorMatrix(): Matrix {
                        this@TerminalSurface.getLocationOnScreen(anchorLocation)
                        anchorMatrix.setTranslate(anchorLocation[0].toFloat(), anchorLocation[1].toFloat())
                        return anchorMatrix
                    }

                    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
                        // 交付不按焦点状态整段拒收（见 onWindowFocusChanged 注释）：
                        // 只有切换瞬间的有界抑制窗口会让输入法重试——返回 false 时
                        // 组字基线不得清、字节不得吞，才能靠重发恢复（N1-24）。
                        if (System.nanoTime() < suppressUntilNanos) return false
                        val newComposing = text?.toString() ?: ""
                        // 纯校对逻辑（ComposingDiff），已单元测试
                        // ——增长/回退/全量重写三种情况集中在一处。
                        val edit = ComposingDiff.reconcile(composing.text, newComposing)
                        // 供自动化输入法验证的锚点——日志序列必须与注入的组字文本一一对应。
                        // 门控在调用处而非只靠 LogUtil.d：release 下可省掉整条消息串的拼接。
                        if (terminal.emulator.BuildConfig.DEBUG) {
                            LogUtil.d(
                                "ComposingDiff",
                                "reconcile prev=${composing.text.length}ch next=$newComposing " +
                                    "bs=${edit.backspaces} app=${edit.append.length}ch",
                            )
                        }
                        if (edit.backspaces > 0) {
                            sendBackspaces(edit.backspaces)
                        }
                        if (edit.append.isNotEmpty()) {
                            encodeAndSend(
                                edit.append,
                                ctrlActive = false,
                                altActive = false,
                            )
                        }
                        composing.compose(newComposing, newCursorPosition)
                        reportComposingCursorAnchor()
                        return true
                    }

                    override fun finishComposingText(): Boolean {
                        // 暂停时不得清基线：PTY 已收到组字增量，失焦清空后输入法
                        // 重新提交会按空基线算出整段追加，同一段文字被写入两次。
                        // 这是基线不变量，与是否接受交付无关——故此处**不**因
                        // 焦点丢失而阻塞交付，只拒绝「清空基线」这一动作。
                        if (isPaused) return false
                        composing.clear()
                        return true
                    }

                    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
                        // 抑制窗口内返回 false 让输入法重发该提交，
                        // 返回 true 等于声称已处理，字符永久丢失（N1-24）。
                        if (System.nanoTime() < suppressUntilNanos) {
                            if (terminal.emulator.BuildConfig.DEBUG) {
                                LogUtil.d(
                                    TAG,
                                    "commitText 在抑制窗口内被拒，" +
                                        "剩余 ${(suppressUntilNanos - System.nanoTime()) / 1_000_000}ms",
                                )
                            }
                            return false
                        }
                        val committedText = text?.toString() ?: return false
                        val terminalViewModel = viewModel
                        val state = terminalViewModel?.state?.value
                        val ctrlActive =
                            state?.ctrlState == ModifierState.Locked || state?.ctrlState == ModifierState.Once
                        val altActive =
                            state?.altState == ModifierState.Locked || state?.altState == ModifierState.Once

                        if (composing.isComposing) {
                            if (committedText == composing.text) {
                                // 已经组字增量转发；不再重发。
                            } else {
                                val clear = ComposingDiff.reconcile(composing.text, "")
                                sendBackspaces(clear.backspaces)
                                encodeAndSend(committedText, ctrlActive, altActive)
                            }
                            composing.clear()
                        } else {
                            encodeAndSend(committedText, ctrlActive, altActive)
                        }
                        // One-shot 修饰键随本次提交一起消费：编码时已按住
                        // ctrl/alt，本次提交后 Once 必须回到 Off，否则粘滞键
                        // 永远不消失。Locked 不受影响（consume 只清 Once）。
                        // 注意：必须在编码之后消费，且只消费一次——
                        // 无修饰提交不清，避免偷走点亮后尚未使用的 Once。
                        if (ctrlActive || altActive) {
                            terminalViewModel.consumeOneShotModifiers()
                        }
                        // 不在此回报锚点：组字刚被清空，锚点的准入条件已不满足，
                        // 任何一次回报都必然被丢弃。
                        return true
                    }

                    /**
                     * 平台已弃用 `ACTION_MULTIPLE` 与 `KeyEvent.characters`，但
                     * compileSdk 37 仍无公开替代面（`InputConnection` 只有
                     * `commitText`，没有 `commitMultipleText`），而硬键盘的多字符
                     * 输入（如蓝牙键盘的粘贴键）**只**经这条通道送达——丢弃它
                     * 等于砍掉该硬件输入。故在此处最小范围抑制并按码点直接提交。
                     */
                    @Suppress("DEPRECATION")
                    override fun sendKeyEvent(event: KeyEvent): Boolean {
                        // 抑制窗口内**拒绝**：返回 true 等于谎报已处理，而事件并未
                        // 送达——硬键盘多字符没有第二条通道，丢字不可恢复，故据实拒绝。
                        // 口径与其余写入路径一致（皆返回 false），且只限这一个有界窗口：
                        // 焦点丢失本身不拒收（见 onWindowFocusChanged 注释）。
                        if (System.nanoTime() < suppressUntilNanos) {
                            return false
                        }
                        return when (event.action) {
                            // 多字符事件只给 characters，没有可映射的键码：
                            // 按码点直接提交，绕开键码映射。
                            KeyEvent.ACTION_MULTIPLE ->
                                commitEventCharacters(event.multipleCharacters())

                            KeyEvent.ACTION_DOWN -> handleKeyEvent(event)

                            else -> true
                        }
                    }

                    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                        // 抑制窗口内返回 false 让输入法重试（与组字/提交同形，N1-24）。
                        if (System.nanoTime() < suppressUntilNanos) return false
                        // beforeLength/afterLength 来自输入法（不可信）：
                        // 负值或巨大值会在主线程上以 NegativeArraySizeException /
                        // OutOfMemoryError 崩溃。组字时钳位到组字缓冲区长度，
                        // 否则钳位到合理的单行上限。
                        // 注意：beforeLength 按码点计数，退格按码点 1:1 发送
                        // （shell 行编辑按字符删除，一个 0x08 删掉整个汉字，
                        // 真机实测锁定：见 CjkBackspaceSemanticsTest）。
                        // 不得按终端列宽加倍（CJK 发 2 个只会多删一个字符）。
                        val maxDeletes = composing.text.length.coerceAtLeast(MAX_SURROUNDING_DELETES)
                        val safeBefore = beforeLength.coerceIn(0, maxDeletes)
                        val safeAfter = afterLength.coerceIn(0, maxDeletes)
                        if (safeBefore > 0) {
                            // 保持组字基线与 PTY 将要持有的内容同步：
                            // setComposingText 的增量逻辑（startsWith/追加/退格分支）
                            // 假定该缓冲区镜像「已提交 + 组字」文本。
                            // 否则组字期间的输入法退格会在此删一次，
                            // 又在 setComposingText 的退格分支再删一次，多吃一个字符。
                            if (composing.isComposing) {
                                // beforeLength 按码点计数（API 33+）；
                                // 删去整个码点（含代理对），使 emoji 与 PTY
                                // 内容保持对齐。退格数与移除码点数 1:1。
                                val removed = composing.dropTrailingCodePoints(safeBefore)
                                val removedBs = ByteArray(removed) { BACKSPACE_BYTE }
                                inputBatchBuffer.write(removedBs)
                            } else {
                                // 非组词直删：safeBefore 已按码点钳制，
                                // shell 按字符删除，1:1 发送。
                                val directBs = ByteArray(safeBefore) { BACKSPACE_BYTE }
                                inputBatchBuffer.write(directBs)
                            }
                        }
                        if (safeAfter > 0) {
                            val del = ByteArray(safeAfter) { DELETE_BYTE }
                            inputBatchBuffer.write(del)
                        }
                        return true
                    }

                    // ── 取文接口：只反映组字基线 ──────────────────────
                    //
                    // `BaseInputConnection` 的默认取自持 editable，而本仓从不读它。
                    // 不覆写这些接口，输入法读到的是空 editable：据此判定字段没有
                    // 内容，于是提交后看不到自己刚写的字，随即整词重发——终端里
                    // 出现重复文本。这是输入法能力「静默缺失」的主因。
                    //
                    // 一律只回答组字基线，绝不掺入屏幕或回滚文本：否则输入法
                    // 会把终端任意位置的字符当成自己的候选区去改写。
                    override fun getTextBeforeCursor(length: Int, flags: Int): CharSequence =
                        composing.textBefore(length.coerceAtMost(MAX_SURROUNDING_QUERY))

                    override fun getTextAfterCursor(length: Int, flags: Int): CharSequence =
                        composing.textAfter(length.coerceAtMost(MAX_SURROUNDING_QUERY))

                    /**
                     * 提取的文本同样只反映组字基线；选区按当前组字光标给出，
                     * 输入法据此显示候选窗口与光标位置。
                     *
                     * `partialEndOffset = -1` 是平台「未选择」的同口径值；填 0 会
                     * 声称选区在文本开头，使输入法把后续编辑当作整段替换。
                     */
                    override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText {
                        val extracted = ExtractedText()
                        extracted.text = composing.text
                        extracted.partialStartOffset = 0
                        extracted.partialEndOffset = if (composing.isComposing) -1 else 0
                        extracted.selectionStart = composing.cursor
                        extracted.selectionEnd = composing.cursor
                        extracted.flags = flags
                        return extracted
                    }

                    /**
                     * 周边文本：`SurroundingText` 是「一段文本 + 选区」的结构，
                     * 故返回整条组字基线并把当前组字光标标为选区。
                     *
                     * 只报基线本身（不掺屏幕文本）：输入法据此在自己写下的范围内
                     * 做上下文判断，而不会把终端任意位置的字符当作自己的内容。
                     */
                    override fun getSurroundingText(beforeLength: Int, afterLength: Int, flags: Int): SurroundingText =
                        SurroundingText(composing.text, composing.cursor, composing.cursor, 0)

                    // ── 替换与改写区域 ──────────────────────────────
                    //
                    // 两者都只允许改写**输入法自己写下的组字基线**：
                    // 输入法可能给出越界下标，而终端里没有可寻址的文本编辑模型，
                    // 越界猜测会让已提交内容被错误删改。故范围不落在基线内即
                    // 返回 false（拒绝）且**发出零字节**，让输入法重试。

                    /**
                     * 把 `[start, end)` 替换为 [text] 并重设光标（API 34+）。
                     *
                     * 目标组字串按「头 + 新文本 + 尾」拼出，再交既有
                     * [ComposingDiff.reconcile] 出口送达，与组字增量同一条路径。
                     */
                    override fun replaceText(
                        start: Int,
                        end: Int,
                        text: CharSequence,
                        newCursorPosition: Int,
                        attrs: TextAttribute?,
                    ): Boolean {
                        // 抑制窗口内返回 false 让输入法重试（与组字/提交同形，N1-24）。
                        if (System.nanoTime() < suppressUntilNanos) return false
                        val target =
                            composing.replaceRange(start, end, text.toString())
                                ?: return false
                        val edit = ComposingDiff.reconcile(composing.text, target)
                        sendBackspaces(edit.backspaces)
                        encodeAndSend(edit.append, ctrlActive = false, altActive = false)
                        composing.compose(target, newCursorPosition)
                        reportComposingCursorAnchor()
                        return true
                    }

                    /**
                     * 把组字区域重锚到 `[start, end)`：只重标定「哪一段正在组字」，
                     * 不引入新文本，区间外的已提交内容必须原样保留。
                     */
                    override fun setComposingRegion(start: Int, end: Int): Boolean {
                        if (System.nanoTime() < suppressUntilNanos) return false
                        val target = composing.reanchorRegion(start, end) ?: return false
                        val edit = ComposingDiff.reconcile(composing.text, target)
                        sendBackspaces(edit.backspaces)
                        if (edit.append.isNotEmpty()) {
                            encodeAndSend(edit.append, ctrlActive = false, altActive = false)
                        }
                        // 重锚后光标落在新区间末尾。
                        composing.compose(target, newCursorPosition = 1)
                        reportComposingCursorAnchor()
                        return true
                    }

                    /**
                     * 按 [KeyEvent.characters] 的码点直接提交，不经键码映射。
                     *
                     * 与 `commitText` 的区别只在于修饰键来源：`ACTION_MULTIPLE`
                     * 是硬键盘发来的，输入法无法给出粘滞修饰键状态，故按裸文本提交。
                     */
                    private fun commitEventCharacters(characters: CharSequence?): Boolean {
                        val text = characters?.toString().orEmpty()
                        if (text.isEmpty()) return true
                        val terminalViewModel = viewModel
                        val state = terminalViewModel?.state?.value
                        val ctrlActive =
                            state?.ctrlState == ModifierState.Locked ||
                                state?.ctrlState == ModifierState.Once
                        val altActive =
                            state?.altState == ModifierState.Locked || state?.altState == ModifierState.Once
                        encodeAndSend(text, ctrlActive, altActive)
                        if (ctrlActive || altActive) {
                            terminalViewModel.consumeOneShotModifiers()
                        }
                        return true
                    }

                    // ── 批量编辑与选区折算 ──────────────────────

                    override fun beginBatchEdit(): Boolean {
                        batchEdits.begin()
                        return true
                    }

                    override fun endBatchEdit(): Boolean {
                        batchEdits.end()
                        return true
                    }

                    /**
                     * 把输入法给出的**绝对**选区折成终端可发送的左右移动。
                     *
                     * 只移动光标而不改写组字文本：读回的文本必须逐字符不变，
                     * 否则输入法会把自己的移动误认为内容变更而重发整段。
                     */
                    override fun setSelection(start: Int, end: Int): Boolean {
                        if (System.nanoTime() < suppressUntilNanos) return false
                        val delta = selectionMover.moveTo(end)
                        if (delta.isEmpty) return true
                        repeat(delta.rightward()) { sendCursorMove(KeyEvent.KEYCODE_DPAD_RIGHT) }
                        repeat(delta.leftward()) { sendCursorMove(KeyEvent.KEYCODE_DPAD_LEFT) }
                        return true
                    }

                    /**
                     * 校正请求一律拒绝：6.1 已证默认路径不分发校正，校正文本不可能
                     * 经提交路径送达，受理即谎称生效；拒绝让输入法停止调用更安全。
                     */
                    override fun commitCorrection(info: CorrectionInfo?): Boolean = false

                    /** 合成一次方向键按下并走既有键事件路径（`setSelection` 折算用）。 */
                    private fun sendCursorMove(keyCode: Int) {
                        val now = SystemClock.uptimeMillis()
                        handleKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, keyCode, 0))
                    }
                }
            imeConnection.currentInputConnection = connection
            return connection
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 五之四、选区手柄弹窗
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有选区手柄覆盖层：一个铺满 Surface 的 TYPE_APPLICATION_SUB_PANEL 窗口承载两个泪滴手柄。
     *
     * 为何用单个覆盖层而非两个 WRAP_CONTENT 弹窗：
     * - 拖动时只需在覆盖层内改 View 的 translationX/Y 并 invalidate——
     *   每帧零次 WindowManager IPC（旧的双手柄弹窗方案每个 ACTION_MOVE
     *   要发 2 次 PopupWindow.update 的 binder 事务）；
     * - 单一窗口整体创建/关闭，不会泄漏孤立手柄（旧的 dismiss 异常路径
     *   会把窗口留在屏幕上——即「多个指针始终不消失」的反馈）；
     * - [HandleOverlayLayout.dispatchTouchEvent] 把手柄触摸路由到拖动逻辑，
     *   其余全部转发给终端 Surface（termux TextSelectionPopupView 模式）。
     *
     * 拖动状态（handleDragState/HandleDrag/dragPointerId）保留在外层类
     * ——触摸路径与边缘滚动 runnable 会读取它们。
     */
    inner class SelectionHandles {
        private var overlayPopup: PopupWindow? = null
        private var overlayContent: HandleOverlayLayout? = null
        private val startHandleRect = Rect()
        private val endHandleRect = Rect()

        /**
         * 系统 Material 选区手柄：解析平台主题属性
         * （android.R.attr.textSelectHandleLeft/Right），
         * 使手柄形态是框架自带的泪滴形而非自定义矢量图。
         */
        internal fun resolveSelectionHandleDrawable(left: Boolean): android.graphics.drawable.Drawable? {
            val attr =
                intArrayOf(
                    if (left) {
                        android.R.attr.textSelectHandleLeft
                    } else {
                        android.R.attr.textSelectHandleRight
                    },
                )
            val typedArray = context.theme.obtainStyledAttributes(attr)
            try {
                return typedArray.getDrawable(0)
            } finally {
                typedArray.recycle()
            }
        }

        fun showSelectionHandles(startRow: Int, startCol: Int, endRow: Int, endCol: Int, themeFgColor: Int) {
            val existingContent = overlayContent
            val existingPopup = overlayPopup
            if (existingContent != null && existingContent.streamForwarding) {
                existingContent.dismissOnStreamEnd(existingPopup)
                existingContent.onStreamEnded = {
                    dismissPopupQuietly(existingContent.consumeDeferredDismiss())
                }
                overlayContent = null
                overlayPopup = null
                startHandleRect.setEmpty()
                endHandleRect.setEmpty()
            } else {
                hideSelectionHandlesNow()
            }
            if (startRow < 0 || startCol < 0 || endRow < 0 || endCol < 0) return
            // showAtLocation 需要窗口 token；在 Activity 结束过渡的帧中
            // 视图可能已 detach，调用会抛 BadTokenException。
            // `isAttachedToWindow` 在拆除回调内部仍为 true（见 [viewDetached]），
            // 故两条件都要查——只凭它会把窗口建在正在销毁的 Activity 上。
            if (viewDetached || !isAttachedToWindow) return
            if (width <= 0 || height <= 0) return

            val leftDrawable = resolveSelectionHandleDrawable(left = true)?.mutate() ?: return
            val rightDrawable = resolveSelectionHandleDrawable(left = false)?.mutate() ?: return
            leftDrawable.setTint(themeFgColor)
            rightDrawable.setTint(themeFgColor)
            selectionHandleWidth = leftDrawable.intrinsicWidth

            val content =
                HandleOverlayLayout(
                    leftDrawable,
                    rightDrawable,
                    leftDrawable.intrinsicWidth,
                    leftDrawable.intrinsicHeight,
                )
            overlayContent = content
            content.onStreamEnded = {
                dismissPopupQuietly(content.consumeDeferredDismiss())
            }

            val loc = IntArray(2)
            getLocationInWindow(loc)
            val popup =
                PopupWindow(content, width, height).apply {
                    isClippingEnabled = false
                    setBackgroundDrawable(null)
                    setAnimationStyle(0)
                    // TYPE_APPLICATION_PANEL 而非 SUB_PANEL：覆盖层与选区菜单同在一个
                    // 选区会话里重建/创建，层号必须把次序钉死——PANEL(1001) 低于
                    // SUB_PANEL(1002)，菜单恒在覆盖层之上。两者同类型时按添加顺序叠加，
                    // 覆盖层每次重建都晚于菜单，会把菜单压住：菜单被压住时落在条目上的
                    // 点击穿透到终端、被当成「柄位外的轻击」清掉选区（现象：所有菜单
                    // 按钮失效、点击菜单消失）。
                    setWindowLayoutType(android.view.WindowManager.LayoutParams.TYPE_APPLICATION_PANEL)
                    isSplitTouchEnabled = false
                    // focusable=false 使键盘输入继续流向终端，
                    // 同时覆盖层消费其边界内的触摸（由 dispatchTouchEvent 路由）。
                    isFocusable = false
                    isOutsideTouchable = false
                }
            try {
                popup.showAtLocation(this@TerminalSurface, 0, loc[0], loc[1])
            } catch (exception: Exception) {
                // WindowManager.BadTokenException：Activity 在 isAttachedToWindow
                // 检查与 showAtLocation 之间被 detach。
                LogUtil.w(TAG, "showSelectionHandles: overlay show failed", exception)
                overlayContent = null
                return
            }
            overlayPopup = popup
            positionAllHandles(startRow, startCol, endRow, endCol)
        }

        /**
         * 把一个被拖动的手柄移到其锚定单元格。纯进程内视图更新：
         * translationX/Y + invalidate，无任何 WindowManager IPC。
         */
        internal fun repositionHandle(which: HandleDrag, row: Int, col: Int) {
            val content = overlayContent ?: return
            val viewportTopGrid = currentViewportTopGrid()
            val visibleRow = (row - viewportTopGrid).coerceIn(0, rows - 1)
            val anchorCol = if (which == HandleDrag.START) col else col + 1
            val (anchorXF, anchorYF) =
                gridToScreen(
                    visibleRow + 1,
                    anchorCol,
                    viewportTopGrid = 0,
                    cellWidth = cellWidth,
                    cellHeight = cellHeight,
                )
            content.position(which, Math.round(anchorXF), Math.round(anchorYF))
            updateHitRect(which, Math.round(anchorXF), Math.round(anchorYF))
        }

        private fun positionAllHandles(startRow: Int, startCol: Int, endRow: Int, endCol: Int) {
            val viewportTopGrid = currentViewportTopGrid()
            val visibleStartRow = (startRow - viewportTopGrid).coerceIn(0, rows - 1)
            val (sx, sy) =
                gridToScreen(
                    visibleStartRow + 1,
                    startCol,
                    viewportTopGrid = 0,
                    cellWidth = cellWidth,
                    cellHeight = cellHeight,
                )
            val visibleEndRow = (endRow - viewportTopGrid).coerceIn(0, rows - 1)
            val (ex, ey) =
                gridToScreen(
                    visibleEndRow + 1,
                    endCol + 1,
                    viewportTopGrid = 0,
                    cellWidth = cellWidth,
                    cellHeight = cellHeight,
                )
            val content = overlayContent ?: return
            content.position(HandleDrag.START, Math.round(sx), Math.round(sy))
            content.position(HandleDrag.END, Math.round(ex), Math.round(ey))
            updateHitRect(HandleDrag.START, Math.round(sx), Math.round(sy))
            updateHitRect(HandleDrag.END, Math.round(ex), Math.round(ey))
        }

        /** 两条定位路径共用的锚点计算（termux hotspot）：START 悬于其单元格角的左下，END 在右下。 */
        private fun updateHitRect(which: HandleDrag, anchorX: Int, anchorY: Int) {
            val handleW = selectionHandleWidth
            if (handleW == 0) return
            val content = overlayContent ?: return
            val handleH = content.handleHeight
            val handleLeft =
                (anchorX - (if (which == HandleDrag.START) (handleW * 3) / 4 else handleW / 4)).coerceIn(
                    0,
                    (width - handleW).coerceAtLeast(0),
                )
            val handleTop = anchorY.coerceIn(0, (height - handleH).coerceAtLeast(0))
            val rect = if (which == HandleDrag.START) startHandleRect else endHandleRect
            rect.set(handleLeft, handleTop, handleLeft + handleW, handleTop + handleH)
            rect.inset(-handleW / 4, -handleH / 4)
        }

        fun hideSelectionHandles() {
            val content = overlayContent
            if (content?.streamForwarding == true) {
                content.dismissOnStreamEnd(overlayPopup)
                return
            }
            hideSelectionHandlesNow()
        }

        /** 手柄内容高度（菜单定位避让同源；未知时 0）。 */
        fun contentHandleHeight(): Int = overlayContent?.handleHeight ?: 0

        private fun hideSelectionHandlesNow() {
            val popup = overlayPopup
            overlayPopup = null
            overlayContent = null
            startHandleRect.setEmpty()
            endHandleRect.setEmpty()
            dismissPopupQuietly(popup)
        }

        private fun dismissPopupQuietly(popup: android.widget.PopupWindow?) {
            if (popup == null) return
            try {
                popup.dismiss()
            } catch (exception: Exception) {
                LogUtil.w(TAG, "dismissPopupQuietly: dismiss failed; forcing remove", exception)
            }
            try {
                val wm =
                    context.getSystemService(android.content.Context.WINDOW_SERVICE)
                        as android.view.WindowManager
                wm.removeViewImmediate(popup.contentView)
            } catch (_: Exception) {
                // 已被 dismiss 移除——无害忽略。
            }
        }

        /**
         * 铺满 Surface 的透明容器，承载两个泪滴手柄；手柄纯靠 translationX/Y 定位。
         * 触摸路由：
         * - DOWN 落在扩大的手柄命中矩形内会锁定该手柄的拖动（按手柄加锁，
         *   另一个手柄的事件流无法抢走——根因 C3 的修复）；
         * - 其余全部事件流原样转发给终端 Surface，
         *   使轻击/滑动/双指缩放手势与没有覆盖层时完全一致。
         */
        private inner class HandleOverlayLayout(
            private val leftDrawable: android.graphics.drawable.Drawable,
            private val rightDrawable: android.graphics.drawable.Drawable,
            val handleWidth: Int,
            val handleHeight: Int,
        ) : android.widget.FrameLayout(context) {
            private val startView = HandleView(leftDrawable)
            private val endView = HandleView(rightDrawable)

            /** 该布局当前拖拽所属的手柄；无拖拽时为 `null`。 */
            var dragOwner: HandleDrag? = null
            private var dragPointerLocked: Int? = null
            var streamForwarding: Boolean = false
                private set

            private var popupDeferredDismiss: android.widget.PopupWindow? = null
            var onStreamEnded: (() -> Unit)? = null

            fun dismissOnStreamEnd(popup: android.widget.PopupWindow?) {
                popupDeferredDismiss = popup
            }

            fun consumeDeferredDismiss(): android.widget.PopupWindow? {
                val pendingPopup = popupDeferredDismiss
                popupDeferredDismiss = null
                return pendingPopup
            }

            init {
                addView(startView, LayoutParams(handleWidth, handleHeight))
                addView(endView, LayoutParams(handleWidth, handleHeight))
            }

            fun position(which: HandleDrag, anchorX: Int, anchorY: Int) {
                val view = if (which == HandleDrag.START) startView else endView
                val targetX =
                    (anchorX - (if (which == HandleDrag.START) (handleWidth * 3) / 4 else handleWidth / 4))
                        .coerceIn(0, (this@TerminalSurface.width - handleWidth).coerceAtLeast(0))
                val targetY =
                    anchorY.coerceIn(0, (this@TerminalSurface.height - handleHeight).coerceAtLeast(0))
                view.translationX = targetX.toFloat()
                view.translationY = targetY.toFloat()
                view.invalidate()
            }

            override fun dispatchTouchEvent(event: MotionEvent): Boolean = when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> routeDown(event)

                MotionEvent.ACTION_MOVE -> routeMove(event)

                // 包含 ACTION_POINTER_UP：当所属手指抬起而第二根仍按下时，
                // 事件流送来的是 POINTER_UP（而非 UP）
                // ——遗漏它会让拖动永久锁定，边缘滚动还可能自行运转。
                MotionEvent.ACTION_UP,
                MotionEvent.ACTION_POINTER_UP,
                MotionEvent.ACTION_CANCEL,
                -> routeStreamEnd(event)

                // 其他指针事件（POINTER_DOWN 等）一律吞掉：
                // 拖动或转发流被占用期间，第二根手指绝不能开启第二条流。
                else -> true
            }

            /** DOWN：锁定手柄拖动，或开始转发终端事件流。 */
            private fun routeDown(event: MotionEvent): Boolean {
                val touchX = event.x.toInt()
                val touchY = event.y.toInt()
                val which = handleAt(touchX, touchY)
                if (which != null) {
                    dragOwner = which
                    dragPointerLocked = event.getPointerId(event.actionIndex)
                    streamForwarding = false
                    latchDragAnchor(which, dragPointerLocked)
                } else {
                    dragOwner = null
                    dragPointerLocked = null
                    streamForwarding = true
                    return this@TerminalSurface.dispatchTouchEvent(event)
                }
                return true
            }

            private fun handleAt(xPx: Int, yPx: Int): HandleDrag? = when {
                !startHandleRect.isEmpty() && startHandleRect.contains(xPx, yPx) -> HandleDrag.START
                !endHandleRect.isEmpty() && endHandleRect.contains(xPx, yPx) -> HandleDrag.END
                else -> null
            }

            private fun lockedIndex(event: MotionEvent): Int = dragPointerLocked?.let {
                event.findPointerIndex(
                    it,
                )
            } ?: -1

            /** MOVE：由本布局拥有的拖拽驱动；吞掉无关指针；否则转发。 */
            private fun routeMove(event: MotionEvent): Boolean {
                if (dragOwner != null) {
                    val lockedIdx = lockedIndex(event)
                    if (lockedIdx >= 0) {
                        driveHandleDragMove(event.getX(lockedIdx), event.getY(lockedIdx))
                    }
                    // 锁定的手指已消失：吞掉来自任何其他指针的游移（多指防漂移）。
                    return true
                }
                if (streamForwarding) {
                    return this@TerminalSurface.dispatchTouchEvent(event)
                }
                return true
            }

            /** UP/CANCEL：仅当所属指针抬起时结束已接管的拖动；否则转发。 */
            private fun routeStreamEnd(event: MotionEvent): Boolean {
                if (dragOwner != null) {
                    // 仅当抬起的指针就是锁定的持有者时才结束拖动：
                    // ACTION_POINTER_UP 会携带所有仍按下的指针，
                    // 故单靠 lockedIndex>=0 检查会在「第二根手指抬起而持有者
                    // 仍在拖动」时错误地结束拖动。
                    val endedByOwner =
                        event.actionMasked == MotionEvent.ACTION_CANCEL ||
                            event.getPointerId(event.actionIndex) == dragPointerLocked
                    if (endedByOwner) {
                        // UP 携带最终触点：快速拖动时最后一个 MOVE 常落后一两格，
                        // 少了这一帧提交的就是旧位置——用户侧表现正是「拖动不跟手」。
                        // 只在 UP 应用（CANCEL 无可靠坐标）。
                        if (event.actionMasked == MotionEvent.ACTION_UP) {
                            val lockedIdx = lockedIndex(event)
                            if (lockedIdx >= 0) {
                                driveHandleDragMove(event.getX(lockedIdx), event.getY(lockedIdx))
                            }
                        }
                        finishHandleDrag()
                        dragOwner = null
                        dragPointerLocked = null
                    }
                    return true
                }
                if (streamForwarding) {
                    val handled = this@TerminalSurface.dispatchTouchEvent(event)
                    if (
                        event.actionMasked == MotionEvent.ACTION_UP ||
                        event.actionMasked == MotionEvent.ACTION_CANCEL
                    ) {
                        streamForwarding = false
                        onStreamEnded?.invoke()
                    }
                    return handled
                }
                return true
            }

            private inner class HandleView(private val drawable: android.graphics.drawable.Drawable) : View(context) {
                override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
                    setMeasuredDimension(handleWidth, handleHeight)
                }

                override fun onDraw(canvas: android.graphics.Canvas) {
                    drawable.setBounds(0, 0, drawable.intrinsicWidth, drawable.intrinsicHeight)
                    drawable.draw(canvas)
                }
            }
        }
    }

    companion object {
        private const val TAG = "TerminalSurface"

        /** 度量未就绪时的兜底网格上界：手机上的终端最多几百列。 */
        private const val MAX_FALLBACK_GRID_SIZE = 512
        private const val MENU_BAR_CORNER_RADIUS_DP = 8
        private const val MENU_ELEVATION_DP = 6
        private const val MENU_ITEM_TEXT_SIZE_SP = 14f
        private const val MENU_ITEM_HORIZONTAL_PADDING_DP = 16
        private const val MENU_ITEM_VERTICAL_PADDING_DP = 12
        private const val MENU_ESTIMATED_WIDTH_DP = 184
        private const val MENU_HEIGHT_DP = 44
        private const val HALF_PIXEL_OFFSET = 0.5f

        private const val ZOOM_THRESHOLD_LOW = 0.9f
        private const val ZOOM_THRESHOLD_HIGH = 1.1f

        // ⑥ 双指缩放：预览边界与 TerminalScreen 的 FONT_SIZE 钳位一致；
        // 预览频率为每秒数次，使塑形手感平滑而无需逐帧重排 ghostty
        // （模拟器上帧基线 41ms）。
        private const val ZOOM_PREVIEW_INTERVAL_NANOS = 60_000_000L // 60ms

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
        private const val IME_RESIZE_DEBOUNCE_MS = 48L
        private const val SCROLLBACK_QUERY_THROTTLE_NANOS = 100_000_000L // 10 Hz

        /** 组字锚点查询限频：与回滚长度同款，锚点位置滞后一窗不影响候选窗口落点。 */
        private const val CURSOR_ANCHOR_QUERY_THROTTLE_NANOS = 100_000_000L // 10 Hz

        // 单次触摸手势转发的滚轮行数上限：无界 repeat 会在主线程逐行同步
        // 等待原生查询（VT 忙时每行最长 500ms），大幅滑动即冻结 UI。
        // 超限行数由后续手势事件携带新坐标补发——丢弃旧坐标而非阻塞等待。
        private const val MAX_WHEEL_LINES_PER_GESTURE = 8

        private const val FALLBACK_CELL_WIDTH = 8f
        private const val FALLBACK_CELL_HEIGHT = 16f
        private const val BACKSPACE_BYTE = 0x08.toByte()
        private const val DELETE_BYTE = 0x7F.toByte()

        // deleteSurroundingText 参数的上界（输入法输入不可信）。
        // 4096 足以覆盖「全选删除大块已提交内容」：256 左 > 256 字符的选区
        // （半删除），同时仍限定了 PTY 写入大小。
        private const val MAX_SURROUNDING_DELETES = 4096

        /**
         * 取文类接口的查询长度上界（与 [MAX_SURROUNDING_DELETES] 同量级）。
         *
         * 输入法给出的 length 不可信；组字基线本身只是一条候选区，
         * 超出的部分按截断返回而非构造超长字符串。
         */
        private const val MAX_SURROUNDING_QUERY = 4096

        /**
         * `setSelection` 单次折算的最大位移（码点）。
         *
         * 输入法给的是它自己整段文本里的绝对下标，与终端内容毫无关系；
         * 不钳位会按一个荒谬的距离向 PTY 灌入海量方向键。
         */
        private const val MAX_SELECTION_MOVE = 4096

        /** 边缘滚动单步方向与行数：+1 向上（回滚多露一行）、-1 向下（少露一行）。 */
        private const val EDGE_SCROLL_STEP_UP = 1
        private const val EDGE_SCROLL_STEP_DOWN = -1
    }

    private fun getAccentColor(): Int = viewModel?.runtime?.accentColor ?: 0xFF2196F3.toInt()

    private var viewModel: TerminalViewModel? = null

    /** 绑定宿主视图模型：Surface 回调内的全部运行期调用经此进入。 */
    fun attachViewModel(viewModel: TerminalViewModel) {
        this.viewModel = viewModel
        // 选区菜单粘贴经批缓冲异步写（N1-26）：与长按粘贴同一出口，
        // 避免主线程逐块同步写 PTY。
        viewModel.pasteSink = { sessionId, data ->
            // 「粘贴内容没到 shell」必须能从日志判定：本行记入队，[InputBatchBuffer]
            // 记录刷写失败或丢弃，`executePaste` 记跳过——三段合起来覆盖整条链路。
            LogUtil.d("TerminalSurface", "粘贴入队 ${data.size} 字节 -> 会话 $sessionId")
            inputBatchBuffer.write(data, sessionId)
        }
        // 硬件按键与鼠标事件同样排到批缓冲（理由见 [installPtyWriteQueue]）。
        installPtyWriteQueue()
        observeImeShiftInputs(viewModel)
    }

    /**
     * 订阅决定平移量与网格的输入：内容下沿（按内容裁剪平移量）、单元格度量
     * （字号与捏合缩放改行高）、备用屏状态（平移恒 0 且需要一次网格重排）。
     * 前两者由渲染线程逐帧发布，故平移量随内容增长即时跟进。
     *
     * 必须在每次 attach 时重建（见 [onAttachedToWindow]）：detach 会停掉它们，
     * 而视图复用不重走 [attachViewModel]。
     */
    private fun observeImeShiftInputs(viewModel: TerminalViewModel) {
        imeShiftJob?.cancel()
        imeShiftJob =
            viewModel.viewModelScope.launch {
                // 三个输入各自足以改变平移量或网格：内容下沿（按内容裁剪）、单元格
                // 行列尺寸（字号/捏合；行高变了但行号没变时内容下沿不会发射）、
                // 备用屏（位移恒 0 且需要一次网格重排）。
                launch { viewModel.runtime.lastContentRowFlow.collect { applyImeShift() } }
                launch { viewModel.runtime.cellMetricsFlow.collect { applyImeShift() } }
                launch {
                    viewModel.runtime.altScreenActiveFlow.collect {
                        applyImeShift()
                        scheduleImeGridResize()
                    }
                }
            }
    }

    @Volatile private var rows: Int = DEFAULT_GRID_ROWS

    @Volatile private var cols: Int = DEFAULT_GRID_COLS
    private var surfaceWidthPixels: Int = 0
    private var surfaceHeightPixels: Int = 0
    private var isScrolling: Boolean = false
    private var scrollAccumulatorPx: Float = 0f

    @Volatile private var scrollOffset: Int = 0
    private var lastImeVisible: Boolean = false

    // 回滚长度缓存：正常情况下直接读渲染线程随帧发布的原子量（零等待），
    // 只有首帧尚未发布时才回退到同步 JNI 查询；回退路径仍按 ~10Hz 节流，
    // 因为 VT 线程忙于解析大块写入时那次查询最多可阻塞约 1s。
    @Volatile private var cachedScrollbackLength: Int = 0

    private var lastScrollbackQueryNanos: Long = 0L

    /** 组字光标锚点最后一次查询的时刻，用于给同步 VT 查询限频。 */
    private var lastCursorAnchorQueryNanos: Long = 0L

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
    private var lastConfiguredWidth = 0
    private var lastConfiguredHeight = 0
    private var pendingSurfaceResize: Runnable? = null

    /**
     * 输入法遮挡高度（px，已扣除系统导航条），由平台 insets 派发维护
     * （[installImeInsetListener]）。本视图用它算平移量（[applyImeShift]）；
     * 网格高度不读这个字段，而是读运行期的 [TerminalRuntime.imeGridReserve]——
     * 那是网格扣减的唯一来源，与本字段同源但不可独立修改。
     *
     * 备用屏应用按整屏行数布局（helix/vim/less 都进备用屏并占满视口），键盘遮挡
     * 的下半屏永远不可见：状态行消失、光标可能落在被遮住的几行里，而应用收不到
     * SIGWINCH 也不会重排——即 DESIGN「输入法弹出时终端（包括 helix 等 tui 应用）
     * 正确匹配窗口大小」不成立。扣除遮挡高度即让网格缩到可见高度，触发一次
     * resize/SIGWINCH，全屏 TUI 随之按新窗口重绘；网格顶对齐渲染，末行紧贴键栏
     * 顶边，与 Termux `adjustResize` 的观感一致。
     */
    private var imeInsetPx: Int = 0
    private var pendingImeGridResize: Runnable? = null
    private var imeShiftJob: Job? = null

    /** 已应用的本视图平移量，避免重复赋值触发无谓的重绘失效。 */
    private var appliedImeShiftPx: Int = 0

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
    private var imeShiftRevealPx: Float = 0f

    /** [imeShiftRevealPx] 的容量上限：当前完整位移（无键盘、备用屏恒 0）。 */
    private var imeShiftCapacityPx: Int = 0

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
    private fun installImeInsetListener() {
        ViewCompat.setOnApplyWindowInsetsListener(this) { view, insets ->
            val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val navigationBottom =
                insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            val reserved = maxOf(imeBottom - navigationBottom, 0)
            if (reserved != imeInsetPx) {
                imeInsetPx = reserved
                LogUtil.d(TAG, "setImeInsetPx: $reserved")
                applyImeShift()
                // 键栏（Compose 覆盖层）也按此高度上移。它只能走组合，故经运行期流发布：
                // 直接回调会在 insets 遍历内写组合状态，那次写入不保证被观察到。
                // 同一次发布也更新网格扣减量（三条网格路径共用，见 imeGridReserve）。
                viewModel?.runtime?.publishImeInsetPx(reserved, this)
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
                viewModel?.clearSelection()
                selectionHandles.hideSelectionHandles()
                hideSelectionMenu("imeVisibilityChanged")
            }
            insets
        }
    }

    /**
     * 把输入法跟随位移直接施加到本视图的 `translationY`。
     *
     * 为什么不平移 Compose 容器（历史做法）：位移要走「组合重组 → 重新测量 →
     * 重新布局」才生效，而在主线程被渲染阻塞时会滞后十几秒（实测见
     * `TerminalRuntime.imeInsetFlow`），期间键盘已经弹出而内容一动不动
     * ——`ImePopupPixelInstrumentedTest` 三个用例即以此判红（位移=0）。
     * `translationY` 是视图自身的属性，在 insets 回调里同步生效，不依赖任何
     * 组合往返。
     *
     * 位移量仍是 [computeImeSurfaceShift] 的判定结果（按内容下沿裁剪、备用屏恒 0），
     * 口径与单测一致。六个触发点各自覆盖它的一个输入：输入法遮挡（insets 派发）、
     * 内容下沿、单元格度量、备用屏状态（后三者由 [observeImeShiftInputs] 订阅）、
     * 视口尺寸（[onSizeChanged]，旋转时 Activity 不重建）、滚动位置
     * （[setScrollOffset]，见 [imeShiftRevealPx]）。
     */
    private fun applyImeShift() {
        val runtime = viewModel?.runtime ?: return
        if (height <= 0) return
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
                surfaceHeightPx = height,
                modifierBarHeightPx = runtime.modifierBarHeightPx,
                imeBottomPx = imeInsetPx,
                isAltScreenActive = runtime.altScreenActiveFlow.value,
            )
        // 显出量不得越过「回到底部还需上滑的行程」：偏移到 0（实时视图）必须归零，
        // 否则提示符被键盘吞掉（TESTING.md 第 37 条）。到回滚区顶端时该上界等于整段
        // 回滚，恒大于容量，不参与限制。
        imeShiftRevealPx =
            imeShiftRevealPx.coerceIn(0f, minOf(capacity.toFloat(), scrollOffset * cellHeight))
        imeShiftCapacityPx = capacity
        val shift = capacity - imeShiftRevealPx.toInt()
        if (shift == appliedImeShiftPx) return
        appliedImeShiftPx = shift
        translationY = -shift.toFloat()
    }

    /**
     * 写入视口滚动位置并向宿主通报。位移的显出量上界依赖滚动位置，故同步刷新位移。
     */
    private fun setScrollOffset(target: Int) {
        if (target == scrollOffset) return
        scrollOffset = target
        applyImeShift()
        onScrollChanged?.invoke(target)
    }

    /**
     * 把一次滚动增量落到视图：偏移、位移显出量、渲染器亚行余量。
     *
     * 拖动与惯性共用这一条管道（见 [applyScrollDistance]），两者的区别只在增量
     * 从哪来——手指还是 `OverScroller`。分成两份实现时惯性那份会漏掉边界丢弃与
     * 显出量，内容按整行跳（STATUS ⑩）。
     */
    private fun applyScrollStep(step: ScrollStep, scrollbackLen: Int) {
        scrollAccumulatorPx = step.newAccumulatorPx
        imeShiftRevealPx = step.revealPx
        if (step.newOffset != scrollOffset) {
            setScrollOffset(step.newOffset)
        } else {
            // 偏移未变，但显出量可能刚在回滚区顶端之外被消费/回收。
            applyImeShift()
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
     * 输入法遮挡或备用屏状态变化后按可见高度重排网格。Surface 尺寸本身不变
     * （窗口是 `adjustNothing`），故只重算网格、不重配交换链。
     *
     * 防抖到稳定高度：键盘动画期间平台逐帧派发 insets，逐帧 resize 会连续发
     * SIGWINCH 让全屏 TUI 反复重排（掉帧、撕裂、耗电）。
     */
    private fun scheduleImeGridResize() {
        if (surfaceWidthPixels <= 0 || surfaceHeightPixels <= 0) return
        val runtime = viewModel?.runtime ?: return
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
        pendingImeGridResize?.let { removeCallbacks(it) }
        pendingImeGridResize = null
        pendingImeGridResizeToken?.let { runtime.releaseRenderPause(it) }
        pendingImeGridResizeToken = token
        pendingImeGridResize =
            Runnable {
                pendingImeGridResize = null
                resizeManager.applyGridResize(surfaceWidthPixels, surfaceHeightPixels)
                // 凭证已陈旧时（视图 detach 后的 `mRunQueue` 延迟派发）ledger 会把它当
                // 空操作，故这里只清仍属于本次的那一个。
                token?.let { viewModel?.runtime?.releaseRenderPause(it) }
                if (pendingImeGridResizeToken == token) pendingImeGridResizeToken = null
            }.also { postDelayed(it, IME_RESIZE_DEBOUNCE_MS) }
    }

    /**
     * 在途 [pendingSurfaceResize] 的暂停凭证；取消或执行后 MUST 归还并清空。
     *
     * 账本由运行期持有（`TerminalRuntime.acquireRenderPause` / `releaseRenderPause`）：
     * 设置页、切后台、Surface 销毁与本文件的两个防抖共用同一本，否则各写同一个
     * 非计数布尔，先结束的那一个会把别人还在等的暂停放掉。不变量见
     * [RenderPauseLedger]。
     */
    private var pendingSurfaceResizeToken: Long? = null

    /** 在途 [pendingImeGridResize] 的暂停凭证（主屏不暂停，故可为空）。 */
    private var pendingImeGridResizeToken: Long? = null

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
    private fun forceResumeRendering() {
        cancelDebounceHolds()
        // 立刻恢复渲染：本函数的所有调用点都刚拿到可用 Surface，故作废生命周期持有
        // （Surface 销毁、切后台）而不只是自己的——残留持有会让后续领用永远发不出
        // 暂停。是否真的恢复由账本计数决定（设置页仍在展示时计数非零，仍保持暂停）：
        // 那是界面层的独立意图，终端视图在设置覆盖层之下仍会收到尺寸变化。
        viewModel?.runtime?.clearLifecycleRenderPause()
    }

    /**
     * 取消两个在途防抖并归还其持有的渲染暂停。
     *
     * 切后台必须调它：SurfaceView 在后台保留窗口，`surfaceDestroyed` 不会来，
     * 而在途防抖仍会到期执行——它经 `applyResizeNormal` 恢复渲染，于是刚装上的
     * 后台暂停被自己的防抖撤掉，渲染线程转去已在被系统回收的 BufferQueue 上取帧
     * （ERROR_SURFACE_LOST_KHR → 返回后永久黑屏）。
     */
    fun cancelResizeDebounces() {
        cancelDebounceHolds()
        pendingImeGridResize?.let { removeCallbacks(it) }
        pendingImeGridResize = null
    }

    private fun cancelDebounceHolds() {
        pendingSurfaceResize?.let { removeCallbacks(it) }
        pendingSurfaceResize = null
        // 必须归还而不是只置空：清掉字段不等于归还持有，一次漏还即让账本里
        // 永远留着一次暂停——此后 acquire() 不再发出暂停（旗标已是真，
        // 布尔幂等），而 release() 也再不会让它归零，所有防抖与设置页的暂停
        // 就此静默失效。只归还本视图领到的这两个凭证：其余持有（设置页、
        // 切后台、Surface 销毁）都不属于本视图。
        pendingSurfaceResizeToken?.let { viewModel?.runtime?.releaseRenderPause(it) }
        pendingSurfaceResizeToken = null
        pendingImeGridResizeToken?.let { viewModel?.runtime?.releaseRenderPause(it) }
        pendingImeGridResizeToken = null
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
                // 在抽屉关闭动画时长（300ms）内抑制「轻击清除」，
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
    private var viewDetached = false

    @Volatile private var suppressUntilNanos = 0L

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
    private var ptyWriteQueueBridge: Bridge? = null

    /**
     * 惯性滚动物理：由本视图按 vsync 自驱的指数衰减（[flingOffsetPx]），
     * 不再交给 [android.widget.OverScroller]。[postOnAnimation] 把各步对齐到 vsync。
     *
     * 为什么不用平台 scroller：厂商 ROM 会改写它的样条。真机日志（厂商 ROM）里
     * `fling, after Optimize, velocityY=-14061 … adjust Duration, min to mFinal`
     * 紧跟一行 `force Finished` —— 整段惯性被折叠成一帧落点，一次猛甩直接跳十几行
     * 并戛然而止，用户看到的就是「上下滑动异常并且卡顿」。自建模型是纯函数：
     * 跨设备一致、可单测、没有平台隐藏状态。
     *
     * 行程单位是**像素**：逐帧增量交给拖动同一条管道（[applyScrollStep]），
     * 整行与亚行余量一起算，内容才能按像素跟随惯性；按行驱动时每帧落点被截成
     * 整行，表现为逐行跳动（STATUS ⑩）。上界含键盘位移的显出量容量，
     * 猛甩也能走到「显出被位移裁掉的网格顶部」的行程末端（STATUS ⑧）。
     */
    private val flingStepRunnable = Runnable { doFlingStep() }

    /** 惯性起点（偏移空间像素：`scrollOffset * cellHeight + 亚行余量`）。 */
    private var flingStartPx = 0f

    /** 惯性起点时刻（[SystemClock.uptimeMillis]，与 vsync 同一时基）。 */
    private var flingStartUptimeMs = 0L

    /** 手势抬起时的速度（像素/秒，手势坐标系：手指上移为负）。 */
    private var flingVelocityPxPerS = 0f

    /** 偏移空间的上界：回滚长度 + 键盘位移显出量容量。 */
    private var flingLimitPx = 0f

    private var flingActive = false

    /** 上一惯性帧的像素位置：逐帧差值即本帧行程。 */
    private var lastFlingPx = 0f

    /** 把惯性滚动动画推进一个 vsync 节拍。 */
    private fun doFlingStep() {
        if (!flingActive) return
        val elapsedMs = SystemClock.uptimeMillis() - flingStartUptimeMs
        val travelPx = flingOffsetPx(flingVelocityPxPerS, elapsedMs)
        val targetPx = flingStartPx + travelPx
        val clampedPx = targetPx.coerceIn(0f, flingLimitPx)
        val deltaPx = clampedPx - lastFlingPx
        lastFlingPx = clampedPx
        if (deltaPx != 0f) {
            val scrollbackLen = currentScrollbackLength()
            // 与 onScroll 同符号：手指下移 distanceY 为负、偏移增大（更旧），
            // 而惯性像素增大的方向也是更旧，故取负后再喂。
            applyScrollStep(
                applyScrollDistance(
                    scrollAccumulatorPx,
                    -deltaPx,
                    cellHeight,
                    scrollOffset,
                    scrollbackLen,
                    imeShiftRevealPx,
                    imeShiftCapacityPx.toFloat(),
                ),
                scrollbackLen,
            )
        }
        // 撞到边界（被钳制）或衰减到行程已尽：收尾，不再空转 vsync 回调。
        val reachedBound = clampedPx != targetPx
        val settled =
            elapsedMs >= FLING_MAX_DURATION_MS ||
                kotlin.math.abs(flingOffsetPx(flingVelocityPxPerS, FLING_MAX_DURATION_MS) - travelPx) < FLING_SETTLE_PX
        if (reachedBound || settled) {
            finishFlingAnimation()
            return
        }
        postOnAnimation(flingStepRunnable)
    }

    /** 停止进行中的惯性动画（触点按下、程序化滚动）。
     *  惯性尚未自然结束时，这也会执行收尾语义——清除亚像素余量并触发
     *  onScrollingStateChanged(false)——使中断后渲染循环的 shouldResetScroll
     *  不会被陈旧的滚动激活标志阻塞。收尾本身复用 [finishFlingAnimation]，
     *  两条路径不得漂移：少清一项就会让 scrollActive 残留、阻塞自动回底。 */
    private fun stopFlingAnimation() {
        if (flingActive) {
            finishFlingAnimation()
        }
    }

    private fun finishFlingAnimation() {
        flingActive = false
        removeCallbacks(flingStepRunnable)
        isScrolling = false
        scrollAccumulatorPx = 0f
        viewModel?.runtime?.setScrollRemainderPx(0f)
        onScrollingStateChanged?.invoke(false)
    }

    @JvmField var isAfterLongPress = false

    @JvmField var scaleFactor = 1.0f

    // ⑥ 双指缩放状态：手势以已渲染字号为锚，每次 onScale 插值；
    // 预览推入度量而不 resize，onScaleEnd 时终结一次。
    private var zoomActive = false
    private var zoomBaseFontSizeSp = 0f
    private var lastZoomPreviewNanos = 0L

    internal enum class HandleDrag {
        NONE,
        START,
        END,
    }

    private var handleDragState = HandleDrag.NONE
    private var selectionHandleWidth = 0

    /**
     * 指针 id 锁定（多指针漂移）：在 ACTION_DOWN 时由锁定拖动的那根手指设定
     * （手柄命中测试或手柄弹窗）；后续 ACTION_MOVE 只取该指针在事件中的槽位，
     * 该指针已不在事件里（抬起或被回收）时整帧吞掉。UP/CANCEL 时清除。
     */
    private var dragPointerId: Int? = null

    /** 最后一次拖动结束的 uptimeMillis；驱动 [shouldSuppressTapAfterDragEnd] 的菜单重显保护。 */
    private var lastHandleDragEndUptimeMs = 0L

    // 拖动锚点：拖动开始时被抓住手柄所固定的单元格边界（绝对网格坐标，0 = 回滚顶部）。
    // 拖动增量相对该锚点计算，因为手柄窗口悬于其锚定单元格之下
    // ——直接用触摸像素会解析到边界下方的那一行。
    // 锚点语义对标 termlib 的 applyHandleDrag（Terminal.kt:2294 起）；
    // 交叉翻转由 [SelectionState.applyHandleDrag] 实现并经 SelectionStateTest 覆盖
    // （拖动手柄越过静止手柄时归属权交换，调用方依返回更新被拖柄），此处只负责把触点换算成网格目标。
    private var dragAnchorRow = 0
    private var dragAnchorCol = 0

    // 抓柄时握住的是哪一侧：决定触点列的参考点（首格左缘 / 末格右缘）。
    // 整次拖动内不变——手柄越过另一侧时只换归属；参考点跟着换会让目标列跳一格。
    private var dragAnchorIsStart = false

    /**
     * 鼠标上报中的 TUI（yazi 等）轻点即左键单击：经 Ghostty 鼠标编码器把
     * 按下/抬起各编码一次写入 PTY。编码器在应用未启用鼠标上报时返回 false
     * 且不写 PTY（见 Bridge.encodeMouseEvent），此时返回 false 让轻点照常
     * 走链接/选区/焦点路径；两次编码间会话被重建时可能只送出按下，与
     * termux TerminalView.onUp 的行为一致——远端由自己的超时兜底。
     */
    private fun encodeMouseTap(event: MotionEvent): Boolean {
        val runtime = viewModel?.runtime ?: return false
        val bridge = runtime.bridge() ?: return false
        val modifiers = KeyModifiers.ghosttyMods(event.metaState)
        val cellWidth = runtime.cellWidth
        val cellHeight = runtime.cellHeight
        val press = bridge.encodeMouseEvent(event.x, event.y, 0, 0, modifiers, cellWidth, cellHeight)
        if (!press) {
            return false
        }
        bridge.encodeMouseEvent(event.x, event.y, 1, 0, modifiers, cellWidth, cellHeight)
        return true
    }

    /**
     * 在视口像素 (px, py) 处打开 OSC 8 超链接（若有）。对标 termux 的
     * TerminalView.openLinkAt：经 cellWidth/cellHeight 完成像素→单元格映射，
     * 再查询原生超链接 URI 并启动系统处理器。成功打开链接时返回 true。
     */
    private fun openLinkAt(px: Float, py: Float): Boolean {
        if (cellWidth <= 0f || cellHeight <= 0f) return false
        val col = pixelToCell(px, cellWidth, cols)
        val row = pixelToCell(py, cellHeight, rows)
        val gridRow = currentViewportTopGrid() + row
        val bridge = viewModel?.runtime?.bridge() ?: return false
        val url = bridge.hyperlinkAt(gridRow, col) ?: return false
        if (url.isBlank()) return false
        val uri =
            try {
                url.trim().toUri()
            } catch (urlError: IllegalArgumentException) {
                LogUtil.w(TAG, "openLinkAt: bad URI", urlError)
                return false
            }
        // 协议白名单：终端输出不可信，故只允许打开 http(s)
        // （阻止 OSC 8 中的 intent:/file:/javascript:）。
        val scheme = uri.scheme?.lowercase(java.util.Locale.ROOT)
        if (scheme != "http" && scheme != "https") {
            LogUtil.w(TAG, "openLinkAt: rejected non-http(s) scheme: $scheme")
            return false
        }
        return try {
            val intent =
                android.content
                    .Intent(android.content.Intent.ACTION_VIEW, uri)
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
            context.startActivity(intent)
            true
        } catch (missingHandler: android.content.ActivityNotFoundException) {
            LogUtil.w(TAG, "openLinkAt: no handler for $uri", missingHandler)
            false
        } catch (blockedError: SecurityException) {
            LogUtil.w(TAG, "openLinkAt: blocked for $uri", blockedError)
            false
        }
    }

    private fun dragTargetFromTouch(touchX: Float, touchY: Float): Pair<Int, Int> {
        val viewportTopGrid = currentViewportTopGrid()
        val viewportRow =
            dragTargetRow(
                anchorViewportRow = dragAnchorRow - viewportTopGrid,
                touchY = touchY,
                cellHeight = cellHeight,
                rows = rows,
            )
        val col =
            dragTargetColumn(
                anchorCol = dragAnchorCol,
                anchorIsStart = dragAnchorIsStart,
                touchX = touchX,
                cellWidth = cellWidth,
                cols = cols,
            )
        val gridRow = viewportTopGrid + viewportRow
        return gridRow to snapToWideCharBoundary(gridRow, col)
    }

    /** 在当前选区范围上重新隐藏并显示选区手柄（拖动结束后使用，使手柄吸附到最终单元格）。 */
    private fun reshowSelectionHandles() {
        // 拆除中绝不重建覆盖层：框架在 detach 回调内部仍报告 isAttachedToWindow=true，
        // 建出的窗口挂在正在销毁的 Activity 上并被永久留下（WindowLeaked: PopupDecorView，
        // 实测 88 次）——留在屏幕上的全屏覆盖层会吃掉后续触摸。
        if (viewDetached || !isAttachedToWindow) return
        // 拖动中绝不重建覆盖层：`hideSelectionHandles` 会 dismiss 掉承载本次按压的弹窗，
        // 窗口令牌随之销毁，平台立刻取消手势——后续 MOVE/UP 落到新窗口却已无 DOWN，
        // 表现为按住手柄一瞬即断、抬手不重锚（仪器用例 SelectionDragQuantifiedTest 判红）。
        // 抓柄路径本来就是「先 latchDragAnchor 隐藏菜单、再走 reshow」，必须在这里挡住；
        // 拖动结束时 finishHandleDrag 会自行调用本函数，届时手柄已吸附到最终单元格。
        if (handleDragState != HandleDrag.NONE) return
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

    /**
     * 手柄拖动结束的共用提交——由覆盖层所属的拖动路径
     * （[SelectionHandles.HandleOverlayLayout]）与旧的 Surface 触摸路径共同调用：
     * 把最终选区提交到 Compose 状态、启用 300ms 轻击保护（termux 隐藏保护）、
     * 重新显示已吸附到最终单元格的手柄与工具栏，
     * 并把高亮刷新到 Rust 渲染器。
     */
    internal fun finishHandleDrag() {
        handleDragState = HandleDrag.NONE
        dragPointerId = null
        invalidateWideCharTailCache()
        viewModel?.commitDragBounds()
        viewModel?.endSelection()
        lastHandleDragEndUptimeMs = SystemClock.uptimeMillis()
        reshowSelectionHandles()
        showSelectionMenuForCurrentSelection()
        viewModel?.runtime?.forceRender()
    }

    /**
     * 手柄拖动的 ACTION_MOVE 主体，从 onTouchEvent 抽出（detekt NestedBlockDepth）：
     * 指针锁定的选区更新 + 在视口 [touchX]/[touchY] 处的边缘滚动步进
     * （调用方已把坐标解析到锁定手指的槽位）。
     */
    private fun driveHandleDragMove(touchX: Float, touchY: Float) {
        currentTouchX = touchX
        currentTouchY = touchY

        // 备用屏（TUI）：禁用边缘滚动——该拖动属于远端全屏缓冲，
        // 而非本地回滚（termux TextSelectionCursorController :218-337 语义）。优先级：
        // SCROLL 锁 > 备用屏禁用 > 普通滚动。
        // 使用拖动开始时的快照：MOVE 帧发出零次 JNI 调用。
        val altScreenActive = dragAltScreenSnapshot
        when (edgeScrollDirection(touchY, surfaceHeightPixels.toFloat(), cellHeight)) {
            EdgeScrollDirection.UP -> {
                // 每次触点移动滚 1 行（design 决策 5）：无定时循环，滚动只随手指移动发生。
                if (!altScreenActive) {
                    stepEdgeScroll(EDGE_SCROLL_STEP_UP)
                }
            }

            EdgeScrollDirection.DOWN -> {
                if (!altScreenActive) {
                    stepEdgeScroll(EDGE_SCROLL_STEP_DOWN)
                }
            }

            EdgeScrollDirection.STOP -> {
                val (gridRow, snapCol) = dragTargetFromTouch(touchX, touchY)
                // 快速拖动路径：直接计算边界并重定位手柄，
                // 避开 Compose _state.update → 重组 → 回读的往返
                // ——那正是每次 MOVE 的主要帧瓶颈。
                val result =
                    viewModel?.dragMove(
                        draggingStart = handleDragState == HandleDrag.START,
                        row = gridRow,
                        col = snapCol,
                        cachedMaxRow = (currentScrollbackLength() + rows - 1).coerceAtLeast(0),
                        cachedMaxCol = (cols - 1).coerceAtLeast(0),
                    )
                if (result != null) {
                    handleDragState = if (result.draggingStart) HandleDrag.START else HandleDrag.END
                    selectionHandles.repositionHandle(HandleDrag.START, result.startRow, result.startCol)
                    selectionHandles.repositionHandle(HandleDrag.END, result.endRow, result.endCol)
                }
            }
        }
    }

    /**
     * 边缘滚动单步：触点位于边缘区的每次 MOVE 滚 [step]（[EDGE_SCROLL_STEP_UP] 向上、
     * [EDGE_SCROLL_STEP_DOWN] 向下）恰好 1 行，滚动随手指移动即时发生（无定时循环），
     * 并把拖拽柄钉到新视口的顶/底行后同步两个手柄位置。
     */
    private fun stepEdgeScroll(step: Int) {
        val scrollbackLen = currentScrollbackLength()
        val newOffset = (scrollOffset + step).coerceIn(0, scrollbackLen)
        if (newOffset != scrollOffset) {
            setScrollOffset(newOffset)
            // 向上滚钉新视口顶行，向下滚钉底行。
            val gridRow =
                if (step == EDGE_SCROLL_STEP_UP) {
                    scrollbackLen - newOffset
                } else {
                    scrollbackLen - newOffset + rows - 1
                }
            updateDragHandleForCell(gridRow)
        }
        val selection = viewModel?.state?.value?.selection
        if (selection?.start != null && selection.end != null) {
            selectionHandles.repositionHandle(HandleDrag.START, selection.start.row, selection.start.col)
            selectionHandles.repositionHandle(HandleDrag.END, selection.end.row, selection.end.col)
        }
    }

    /** 把被拖动的手柄移到当前触摸列所在 [gridRow] 的单元格，并在宽字符（CJK）边界处吸附。 */
    private fun updateDragHandleForCell(gridRow: Int) {
        // 列沿用与快速路径同一套折算（锚点 + 手指位移）：边缘滚动区若改用
        // 「手指所在格」，手指进出边缘区时列会跳一格。
        val curCol =
            dragTargetColumn(
                anchorCol = dragAnchorCol,
                anchorIsStart = dragAnchorIsStart,
                touchX = currentTouchX,
                cellWidth = cellWidth,
                cols = cols,
            )
        val snappedCol = snapToWideCharBoundary(gridRow, curCol)
        val flippedSide =
            if (handleDragState == HandleDrag.START) {
                viewModel?.updateSelectionStart(gridRow, snappedCol)
            } else if (handleDragState == HandleDrag.END) {
                viewModel?.updateSelection(gridRow, snappedCol)
            } else {
                null
            }
        if (flippedSide != null) {
            handleDragState = if (flippedSide) HandleDrag.START else HandleDrag.END
        }
        // 节流的原生推送使单元格反色高亮在边缘滚动拖动期间保持实时
        // （与快速 MOVE 路径同一节奏）。
        viewModel?.syncDragSelectionToNativeThrottled()
    }

    /**
     * 把列吸附到宽字符边界：当 `col` 落在宽字符的后半部分时回退一格，
     * 使选区手柄绝不把一个宽字符从中间切开。
     *
     * 判定取自网格单元宽度（原生 [Bridge.wideCharTailCols]）：[Bridge.scrollbackLine]
     * 每列恰好一个字符，宽字符尾格在该字符串里是空格占位，与真空白不可区分——
     * 按文本反推必然出错。
     *
     * 整行的尾格列在一次拖动会话内只查一次：吸附在每个 MOVE 帧都要做，而该查询是
     * 发往 VT 线程的同步往返（空闲时最坏约一个 50ms 查询节拍）。行只经边缘滚动变化，
     * 而那本身会换行号并重新取数。
     */
    private fun snapToWideCharBoundary(gridRow: Int, col: Int): Int {
        if (col <= 0) return col
        val tailCols = wideCharTailCols(gridRow)
        return if (tailCols.contains(col)) col - 1 else col
    }

    /** 拖动会话内 [snapToWideCharBoundary] 的单项缓存：一次取整行，逐帧复用。 */
    private fun wideCharTailCols(gridRow: Int): IntArray {
        val bridge = viewModel?.runtime?.bridge() ?: return IntArray(0)
        if (wideCharDragSession && gridRow == cachedWideCharTailRow) return cachedWideCharTailCols
        val tailCols = bridge.wideCharTailCols(gridRow)
        cachedWideCharTailRow = gridRow
        cachedWideCharTailCols = tailCols
        return tailCols
    }

    private var cachedWideCharTailRow = -1
    private var cachedWideCharTailCols: IntArray = IntArray(0)

    /**
     * 拖动会话进行中：期间只按行号复用缓存。
     *
     * 抬起与开新拖动都必须让缓存失效——行内容在这两次之间可能被新输出改写，
     * 而宽字符吸附依赖的正是该行当前的尾格列。只清标志而保留缓存时，下一次拖动
     * 在同一行号上直接命中上一次拖动的尾格，吸附到早已不存在的边界。
     */
    private var wideCharDragSession = false

    private fun invalidateWideCharTailCache() {
        cachedWideCharTailRow = -1
        cachedWideCharTailCols = IntArray(0)
        wideCharDragSession = false
    }

    private fun latchDragAnchor(which: HandleDrag, pointerId: Int? = null) {
        handleDragState = which
        // 抓柄即隐藏（design 决策 3）：拖动中菜单不遮挡选择；抬手由
        // finishHandleDrag 按新几何重锚重显。
        hideSelectionMenu("latchDragAnchor")
        // 锁定到发起拖动的那根手指：来自其他指针的后续 MOVE 事件
        // 绝不能改变选区。
        dragPointerId = pointerId
        // 按拖动快照化状态，使 MOVE 帧发出零次 JNI 调用。
        dragAltScreenSnapshot =
            runCatchingCancellable { viewModel?.runtime?.bridge()?.isAltScreenActive() ?: false }
                .getOrDefault(false)
        invalidateWideCharTailCache()
        wideCharDragSession = true
        // 在拖动开始时只调用一次 setSelectionDragging(true)，
        // 使渲染线程抑制新输出引起的滚动复位。原先在 dragSelection 内
        // 按每个 MOVE 调用——鉴于它是幂等的，那纯属浪费。
        viewModel?.runtime?.setSelectionDragging(true)
        // 抓住仅粘贴选区的手柄会把它升级为文本选区，
        // 使拖动能扩展范围，而不被「仅粘贴不可变」守卫吞掉。
        viewModel?.beginHandleDragOnPasteOnly()
        val selection = viewModel?.state?.value?.selection
        dragAnchorIsStart = which == HandleDrag.START
        if (which == HandleDrag.START) {
            dragAnchorRow = selection?.start?.row ?: 0
            dragAnchorCol = selection?.start?.col ?: 0
        } else {
            dragAnchorRow = selection?.end?.row ?: 0
            dragAnchorCol = selection?.end?.col ?: 0
        }
    }

    /** 拖动开始时快照的备用屏标志（MOVE 帧绝不做 JNI 调用）。 */
    private var dragAltScreenSnapshot = false

    private var longPressDragging = false
    private var longPressStartX = 0f
    private var longPressStartY = 0f

    /**
     * 长按后是否已越过触摸阈值，转入真正的拖动扩选。
     *
     * 长按选词后手指的微动（长按自身的抖动，真机上必然出现）不该改写刚选出的词：
     * 越过阈值前 ACTION_MOVE 一律不动选区。少了这道守卫，1px 位移就把
     * `(1,6)-(1,10)` 的整词塌缩成 `(1,6)-(1,7)`，菜单随选区重锚，
     * 用户点下去的一瞬正好点空——表象即「所有菜单按钮失效、点一下菜单自己消失」。
     */
    private var longPressDragStarted = false

    /**
     * 本次手势是否已把拖动转发为滚轮（见 onScroll）：转发过的手势其惯性段
     * 同样属于应用，onFling 只吞不滚，绝不让本地回滚接管（见 onFling）。
     * 手势开始时在 onDown 重置。
     */
    private var wheelForwarded = false

    private val clipboardAccess = ClipboardAccess(context, tag = "Surface")
    private val resizeManager = ResizeManager()
    private val imeConnection = ImeConnection()
    private val selectionHandles = SelectionHandles()

    val isSelectingText: Boolean
        get() = viewModel?.state?.value?.selection?.active == true

    private val gestureListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(motionEvent: MotionEvent): Boolean {
                // 手势开始时重置滚轮转发标记：惯性段只属于做出转发的那次手势。
                wheelForwarded = false
                // 手势开始时重置亚单元格累加器，使首次 onScroll 距离从干净的起点算起。
                // 同步本地偏移与运行时真源：渲染线程回底后本地仍旧值，下次手势若从旧值起算会跳变。
                viewModel?.runtime?.activeSessionScrollOffset()?.let {
                    scrollOffset = it
                    // 会话切换连回滚长度一起换掉，位移显出量的上界随之变化（见 applyImeShift）。
                    applyImeShift()
                }
                scrollAccumulatorPx = 0f
                viewModel?.runtime?.setScrollRemainderPx(0f)
                return true
            }

            override fun onShowPress(motionEvent: MotionEvent) {
                isAfterLongPress = false
            }

            override fun onScroll(e1: MotionEvent?, e2: MotionEvent, distanceX: Float, distanceY: Float): Boolean {
                // 选词中移动由 ACTION_MOVE 长按拖动分支消费；此处吞掉避免双路径竞争致摇晃。
                if (isSelectingText) return true
                // 触摸拖动 = 应用滚动：应用启用鼠标上报时经 Ghostty 鼠标编码器
                // 把每个滚过的行转发成一个滚轮事件，对标 termux TerminalView 的
                // scrolledWithFinger——yazi 等主屏 TUI 里拖动即应用内滚动。
                // 应用未启用上报时编码器返回 false（不写 PTY），主屏回落到本地
                // 回滚；备用屏（vim/less/htop）没有回滚，拖动静默吞掉。
                // 与 onGenericMotionEvent 中的外接鼠标路径一致。
                val wheelBridge = viewModel?.runtime?.bridge()
                if (wheelBridge != null && wheelBridge.isMouseTrackingActive()) {
                    val cellWidth = viewModel?.runtime?.cellWidth ?: 1f
                    val cellHeight = viewModel?.runtime?.cellHeight ?: 1f
                    val pointerXPx = e2.x
                    val pointerYPx = e2.y
                    // 走满一个单元格高度 = 一个滚轮行，与下方的本地滚动映射
                    // （distanceY / cellHeight）一致。
                    val lines = kotlin.math.max(1, kotlin.math.abs((distanceY / cellHeight).toInt()))
                    // 手指上移（distanceY > 0）= 滚轮上（3，较旧）；手指下移 = 滚轮下（4，较新）
                    val button = if (distanceY > 0f) 3 else 4
                    val cappedLines = lines.coerceAtMost(MAX_WHEEL_LINES_PER_GESTURE)
                    // 一次往返编码全部档位：逐档 encodeMouseEvent 会把 UI 线程
                    // 按在每档一次 VT 应答上（滚动手势一次可滚十余行）。
                    wheelForwarded =
                        wheelBridge.encodeMouseWheel(
                            pointerXPx,
                            pointerYPx,
                            button,
                            KeyModifiers.ghosttyMods(e2.metaState),
                            cellWidth,
                            cellHeight,
                            cappedLines,
                        )
                }
                if (wheelForwarded) return true
                if (wheelBridge != null && wheelBridge.isAltScreenActive()) return true
                val scrollbackLen = currentScrollbackLength()
                if (!isScrolling) {
                    isScrolling = true
                    onScrollingStateChanged?.invoke(true)
                }
                // 亚单元格累加器：distanceY < cellHeight 绝不能被丢弃，
                // 只发出整行，余量带入下一次 onScroll。
                // 方向：手指下移（distanceY<0，currentY - previousY）→ 更旧的历史
                // （偏移增大，视口顶部行号减小）→
                // 对应 termux TerminalView:onScroll 的 deltaRows = distanceY / lineSpacing
                // 与 doScroll rowsDown>0 → mTopRow+1（更新）、rowsDown<0 → mTopRow-1（更旧）。
                // 即 distanceY 为负(下移)时 deltaRows 为负,对应 older,与本实现 scrollOffset 增加一致。
                // 截断趋向零（toInt）：正/负亚行阈值对称，消除 floor 非对称导致的漂移。
                // 注意符号:distanceY = previousY - currentY,下移为负,需取反累加才能使下移增加偏移。
                val scrollStep =
                    applyScrollDistance(
                        scrollAccumulatorPx,
                        distanceY,
                        cellHeight,
                        scrollOffset,
                        scrollbackLen,
                        imeShiftRevealPx,
                        imeShiftCapacityPx.toFloat(),
                    )
                applyScrollStep(scrollStep, scrollbackLen)
                return true
            }

            override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
                // 选词中 fling 同样吞掉，避免落到滚动路径致视口跳变。
                if (isSelectingText) return true
                // 本次手势的拖动已把滚轮转发给应用：惯性段属于应用，
                // 直接吞掉，绝不让本地回滚接管（见 onScroll 的 wheelForwarded）。
                if (wheelForwarded) return true
                // 在备用屏（vim/less/htop）上，fling 绝不能滚动本地回滚
                // ——该手势属于远端。此处直接丢弃（消费掉）而不转发，
                // 因为惯性速度没有干净的滚轮行数映射；
                // 拖动滚动（onScroll）已经按行转发滚轮事件。
                val flingBridge = viewModel?.runtime?.bridge()
                if (flingBridge != null && flingBridge.isAltScreenActive()) {
                    return true
                }
                val scrollbackLen = currentScrollbackLength()
                val absX = kotlin.math.abs(velocityX)
                val absY = kotlin.math.abs(velocityY)

                // 横向 fling 消费掉但不做任何事：DESIGN 只要求修饰键栏左右滑动，
                // 终端内容区的横向甩动没有声明语义。此前它被映射成向运行中程序注入
                // ESC / Tab（500px/s 的门槛极低），会在 vim/less 或半行命令中直接
                // 破坏用户输入。
                if (absX > absY) {
                    return true
                }

                // velocityY 系像素/秒，偏移空间同样是像素：不再除行高换成行/秒
                // （那样逐帧落点是整行，内容只能逐行跳，STATUS ⑩）。行程起点带上
                // 亚行余量、上界带上键盘位移的显出量容量，见 [flingOffsetPx]。
                stopFlingAnimation()
                isScrolling = true
                onScrollingStateChanged?.invoke(true)
                flingStartPx = scrollOffset * cellHeight + scrollAccumulatorPx
                flingVelocityPxPerS = velocityY
                flingLimitPx = scrollbackLen * cellHeight + imeShiftCapacityPx
                flingStartUptimeMs = SystemClock.uptimeMillis()
                // 起点先夹进范围：不夹的话首帧差值会多出被夹掉的一段、方向相反。
                lastFlingPx = flingStartPx.coerceIn(0f, flingLimitPx)
                flingActive = true
                postOnAnimation(flingStepRunnable)
                return true
            }

            override fun onSingleTapUp(event: MotionEvent): Boolean {
                // 长按抬手不是轻击：直接返回，不触碰选区。
                if (isAfterLongPress) {
                    isAfterLongPress = false
                    longPressDragging = false
                    return true
                }
                val now = SystemClock.uptimeMillis()
                // 300ms 隐藏保护：手柄拖动松手后的首次轻击属于拖动手势的收尾，
                // 而非新的轻击。先于其他判定拦截，避免拖尾关闭刚重显的菜单。
                if (shouldSuppressTapAfterDragEnd(now, lastHandleDragEndUptimeMs)) {
                    return true
                }
                // 抽屉关闭动画会让遮罩轻击穿透到 Surface；
                // 不要把它当作终端轻击（那会清除选区）。
                if (System.nanoTime() < suppressUntilNanos) {
                    return true
                }
                if (isScrolling) {
                    // 只结束滚动状态；绝不要把 scrollOffset 重置为 0，
                    // 否则每次轻击都会撤销用户的滚动，使回滚浏览不可用
                    // （「滚动不起作用」的反馈）。像素余量确实会被重置，
                    // 使视图停在整行而非行中间。
                    isScrolling = false
                    scrollAccumulatorPx = 0f
                    viewModel?.runtime?.setScrollRemainderPx(0f)
                    onScrollingStateChanged?.invoke(false)
                    return true
                }
                // 鼠标上报中的 TUI（yazi 等）：轻点即左键单击，对标 termux
                // TerminalView.onUp 的语义——编码器在应用未启用鼠标上报时返回 false
                // 且不写 PTY，轻点照常走链接/选区/焦点路径。开启上报时点击属于
                // 运行中的应用，链接不抢在它之前。
                if (!isSelectingText && encodeMouseTap(event)) {
                    return true
                }
                // OSC 8 超链接轻点（对标 termux TerminalView.openLinkAt）：
                // 轻点超链接单元格会打开该 URI，而不是拉起键盘。
                if (openLinkAt(event.x, event.y)) {
                    return true
                }
                if (isSelectingText) {
                    selectionHandles.hideSelectionHandles()
                    viewModel?.clearSelection()
                    post {
                        // minSdk 33：可直接使用平台的 WindowInsetsController；ViewCompat 的辅助方法已弃用。
                        val controller = windowInsetsController
                        controller?.hide(
                            android.view.WindowInsets.Type.ime(),
                        )
                    }
                    return true
                }
                viewModel?.clearSelection()
                suppressUntilNanos = 0L
                keyboardRequested = true
                requestFocus()
                post {
                    val controller = windowInsetsController
                    controller?.show(
                        android.view.WindowInsets.Type.ime(),
                    )
                }
                return true
            }

            override fun onLongPress(event: MotionEvent) {
                if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
                isAfterLongPress = true
                longPressDragging = true
                longPressDragStarted = false
                longPressStartX = event.x
                longPressStartY = event.y
                handleLongPress(event.x, event.y)
            }
        }

    private val gestureDetector =
        GestureDetector(context, gestureListener).also {
            // 不支持双击/多击选择：禁用框架双击检测，使每次点击都触发 onSingleTapUp。
            it.setOnDoubleTapListener(null)
        }

    private val scaleDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                    if (isSelectingText) return false
                    zoomBaseFontSizeSp = viewModel?.runtime?.appliedFontSizeSp() ?: return false
                    zoomActive = true
                    // scaleFactor 同时充当长按守卫：双指缩放占用触摸序列期间，onLongPress 会跳过。
                    scaleFactor = 1.0f
                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    if (!zoomActive || isSelectingText) return false
                    scaleFactor *= detector.scaleFactor
                    val runtime = viewModel?.runtime ?: return false
                    val sizeSp =
                        zoomFontSize(
                            zoomBaseFontSizeSp,
                            scaleFactor,
                            runtime.spToPxScale,
                            runtime.screenWidthDp,
                        )
                    val now = System.nanoTime()
                    if (now - lastZoomPreviewNanos >= ZOOM_PREVIEW_INTERVAL_NANOS) {
                        lastZoomPreviewNanos = now
                        onZoomPreview?.invoke(sizeSp)
                    }
                    return true
                }

                override fun onScaleEnd(detector: ScaleGestureDetector) {
                    if (!zoomActive) return
                    zoomActive = false
                    val runtime = viewModel?.runtime ?: return
                    // 落定判定用未钳位值：钳位会把无缩放手势（8sp 基准×1.0→钳制 14sp）
                    // 误判为新尺寸并持久化，未钳位比较只在真实缩放时落定。
                    val rawSizeSp = zoomBaseFontSizeSp * scaleFactor
                    val sizeSp =
                        zoomFontSize(
                            zoomBaseFontSizeSp,
                            scaleFactor,
                            runtime.spToPxScale,
                            runtime.screenWidthDp,
                        )
                    scaleFactor = 1.0f
                    if (zoomSettledOnNewSize(zoomBaseFontSizeSp, rawSizeSp)) {
                        // 手势稳定在新尺寸上：持久化并完整应用（单次网格重排）。
                        onZoomChanged?.invoke(sizeSp)
                    } else {
                        // 回到锚定尺寸：撤销已推入原生度量的预览。
                        onZoomPreview?.invoke(zoomBaseFontSizeSp)
                    }
                }
            },
        )

    fun handleLongPress(xPx: Float, yPx: Float) {
        // 长按 → 词选择。不支持双击/多击选择，词选择只走长按一条路径。
        if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
        isAfterLongPress = true

        performHapticFeedback(HapticFeedbackConstantsCompat.LONG_PRESS)

        selectionHandles.hideSelectionHandles()

        val bridge = viewModel?.runtime?.bridge()
        val rawCol = (xPx / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
        val row = (yPx / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
        val gridRow = currentViewportTopGrid() + row
        // 落点先吸附到字符起始列：宽字符后半格在行文本里是空格占位，不吸附就会被
        // 当成空白而弹出仅粘贴菜单，长按整个字都选不中。
        val col = snapToWideCharBoundary(gridRow, rawCol)

        // 我们则落到单格反色 + 粘贴菜单。
        val line = bridge?.scrollbackLine(gridRow)
        // 空白目标 = 空行、空白单元格，或行尾之后的任何列
        // ——termux 的 getSelectedText(x,y,x,y) 对三者都返回 ""，
        // 故它们都必须归类为仅粘贴。原先的 `col < line.length` 合取条件
        // 把行尾各列归类为文本，这正是「在提示符右侧长按
        // 却弹出带 PASTE 的完整菜单」的根因。
        val isOnWhitespace = isWhitespaceCell(line, col)

        if (isOnWhitespace) {
            viewModel?.startSelection(gridRow, col, pasteOnly = true)
            viewModel?.endSelection()

            // （termux 对等）：空白单元格的选区也显示两个手柄叠在该单元格上
            // ——拖动任一手柄都会从空白处扩展出范围选区，
            // 与 termux 的 setInitialTextSelectionPosition 流程完全一致。
            selectionHandles.showSelectionHandles(gridRow, col, gridRow, col, getAccentColor())

            LogUtil.d(
                "Selection",
                "LONG_PRESS whitespace: row=$row col=$col gridRow=$gridRow " +
                    "menu=PASTE_ONLY",
            )
        } else {
            // 上游 select_word 派生词界（ghostty 默认边界：空白与
            // `'"\`│|:;,()[]{}<>$`），长按走 native 派生；
            // native 侧已安装选区，回传的有序界限驱动状态与控制柄。
            val bounds = resolveWordSelectionBounds(bridge?.selectWordAt(gridRow, col), gridRow, col)

            LogUtil.d(
                "Selection",
                "LONG_PRESS text: tapRow=$row tapCol=$col gridRow=$gridRow " +
                    "expanded start=(${bounds.startRow},${bounds.startCol}) " +
                    "end=(${bounds.endRow},${bounds.endCol}) " +
                    "menu=FULL cellW=$cellWidth cellH=$cellHeight rows=$rows cols=$cols",
            )

            viewModel?.startSelection(bounds.startRow, bounds.startCol)
            viewModel?.updateSelection(bounds.endRow, bounds.endCol)
            viewModel?.endSelection()
            selectionHandles.showSelectionHandles(
                bounds.startRow,
                bounds.startCol,
                bounds.endRow,
                bounds.endCol,
                getAccentColor(),
            )
        }
    }

    private var currentTouchX = 0f
    private var currentTouchY = 0f

    init {
        holder.addCallback(this)
        // SurfaceView 在窗口中开了孔；终端内容由原生渲染器绘制到 Surface，
        // 其余一切（工具栏、覆盖层）留在普通视图层级中。
        holder.setFormat(android.graphics.PixelFormat.RGBA_8888)
        isFocusable = true
        isFocusableInTouchMode = true
        setWillNotDraw(false)
        scaleDetector.isQuickScaleEnabled = false
        // 输入法遮挡高度的唯一来源：平台 insets 派发（见 installImeInsetListener）。
        installImeInsetListener()
    }

    private var keyboardRequested = false

    fun finishComposing() {
        imeConnection.currentInputConnection?.let { ic ->
            ic.finishComposingText()
        }
    }

    override fun onCheckIsTextEditor(): Boolean = keyboardRequested

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
            imeConnection.currentInputConnection?.let { ic ->
                ic.finishComposingText()
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
                val vm = viewModel ?: return@Runnable
                // ON_RESUME 已经发生（只是 Surface 当时未就绪）：切后台那次暂停必须
                // 此时归还，否则账本里永远留着它，此后任何 acquire 都发不出暂停。
                vm.runtime.resumeRenderingFromBackground()
                val surface = holder.surface
                // Surface 就绪后按常规路径恢复：清掉 Surface 销毁那次暂停、重启渲染
                // 线程并出一帧探针。Surface 仍无效时留待下一次 surfaceChanged/resize。
                if (surface.isValid && width > 0 && height > 0) {
                    vm.runtime.clearSurfaceDestroyedRenderPause()
                    vm.runtime.resumeRendering()
                    vm.runtime.forceRender()
                }
            }
                .also { postDelayed(it, delayMillis) }
    }

    private fun currentScrollbackLength(): Int {
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
                LogUtil.e(TAG, "scrollbackLength query failed", error)
                cachedScrollbackLength
            }
        viewModel.updateScrollbackLength(cachedScrollbackLength)
        return cachedScrollbackLength
    }

    /** 视口顶部显示的绝对网格行（scrollbackLength - scrollOffset）：选区矩形与拖动/光标手柄都需要的唯一来源。 */
    private fun currentViewportTopGrid(): Int = currentScrollbackLength() - scrollOffset

    fun scrollToRow(row: Int) {
        stopFlingAnimation()
        val scrollbackLen = currentScrollbackLength()
        val targetOffset = scrollOffsetForRow(row, scrollbackLen)
        if (targetOffset != scrollOffset) {
            setScrollOffset(targetOffset)
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
        stopFlingAnimation()
        lastScrollbackQueryNanos = 0L
        val sessionOffset = viewModel?.runtime?.activeSessionScrollOffset() ?: 0
        if (scrollOffset != sessionOffset) {
            scrollOffset = sessionOffset
            // 会话切换连回滚长度一起换掉，位移显出量的上界随之变化（见 applyImeShift）。
            applyImeShift()
        }
    }

    fun getScrollOffset(): Int {
        // 运行时为真源：新输出在渲染线程将 entry 置 0，若只读本地字段则回车后视图永不回底。
        // 手势期间本地与运行时同步更新（onScroll 双写），此处优先读运行时保证回底可见。
        return viewModel?.runtime?.activeSessionScrollOffset() ?: scrollOffset
    }

    fun getRows(): Int = rows

    fun getCols(): Int = cols

    // 视图 detach 时 close() 会 shutdown 发送线程；同一实例再次 attach（窗口转换、
    // 分屏、Compose 重新挂载）时必须重建，否则每次 write 都抛 RejectedExecutionException
    // ——输入法输入与粘贴会在无任何症状的情况下永久失效。
    private var inputBatchBuffer = newInputBatchBuffer()

    /**
     * 本视图的 Surface 归属凭证：一个视图实例一个。用于区分「当前持有 Surface 的视图」
     * 与「Activity 重建后仍在迟到回调的旧视图」（见 [TerminalRuntime.onSurfaceDestroyed]）。
     */
    private val surfaceOwnershipToken = Any()

    /**
     * 把硬件按键与鼠标事件的写入接到批缓冲。
     *
     * 直写的 `feedPty` 在主端满时最多等 5s，会把 UI 线程冻住；共用一条出口还顺带
     * 保证与输入法写入的先后顺序（两路各写时，缓冲中待发的退格会被直写的击键越过）。
     * attach 与首次绑定都要装，故只此一处实现。
     */
    private fun installPtyWriteQueue() {
        val bridge = viewModel?.runtime?.bridge() ?: return
        bridge.ptyWriteQueue = { sessionId, data ->
            inputBatchBuffer.write(data, sessionId)
        }
        ptyWriteQueueBridge = bridge
    }

    private fun newInputBatchBuffer(): InputBatchBuffer = InputBatchBuffer(
        flushSink = { sessionId, data ->
            // 未绑定时 `viewModel?.` 会把刷写变成无声空操作：粘贴与输入就此消失，
            // 日志里片痕不留。[InputBatchBuffer] 对「已关闭/被拒绝」都留了日志，
            // 这条同样必须留，否则「字节入队了却没到 shell」永远无法从日志判定。
            val target = viewModel
            if (target == null) {
                LogUtil.w("TerminalSurface", "批缓冲刷写丢弃：视图模型尚未绑定 (${data.size} 字节)")
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
    private fun showSelectionMenuForCurrentSelection() {
        val selection = viewModel?.state?.value?.selection
        if (selection == null) {
            LogUtil.d(TAG, "同步备份跳过：视图模型为空")
            return
        }
        if (!selection.active || selection.start == null || selection.end == null) {
            LogUtil.d(TAG, "同步备份跳过：选区未就绪")
            return
        }
        showSelectionMenu(selection.pasteOnly)
    }

    private fun startWordSelectionAt(event: MotionEvent) {
        val col = pixelToCell(event.x, cellWidth, cols)
        val row = pixelToCell(event.y, cellHeight, rows)
        // 上游 select_word 词界 — 与长按完全同一 native 派生、同一回退函数。
        val gridRow = currentViewportTopGrid() + row
        val bounds =
            resolveWordSelectionBounds(
                viewModel?.runtime?.bridge()?.selectWordAt(gridRow, col),
                gridRow,
                col,
            )
        viewModel?.startSelection(bounds.startRow, bounds.startCol)
        viewModel?.updateSelection(bounds.endRow, bounds.endCol)
        // 回退分支同样须收尾：只 startSelection 会把选区永久留在拖动态，
        // 此后输入不再自动贴底。
        viewModel?.endSelection()
        LogUtil.d(
            "Selection",
            "RIGHT_CLICK word: tapRow=$row tapCol=$col " +
                "expanded start=(${bounds.startRow},${bounds.startCol}) " +
                "end=(${bounds.endRow},${bounds.endCol})",
        )

        try {
            magnifier = magnifier ?: Magnifier.Builder(this@TerminalSurface).build()
            magnifier?.show(event.rawX, event.rawY)
        } catch (exception: Exception) {
            LogUtil.w(TAG, "magnifier show failed (non-critical)", exception)
        }
    }

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

    private fun handleKeyEvent(event: KeyEvent): Boolean = onKeyDown(event.keyCode, event)

    // ══════════════════════════════════════════════════════════════════════
    // 四、触摸事件派发
    // ══════════════════════════════════════════════════════════════════════

    override fun performClick(): Boolean = super.performClick()

    // 派发约 15 种彼此不同的手势/意图，涵盖选区、滚动、长按与硬件按键交互。
    override fun onTouchEvent(event: MotionEvent): Boolean {
        val isRelease = event.actionMasked == MotionEvent.ACTION_UP
        // 覆盖整条事件流，不只是抬手：叠加层可见时 Surface 必须把 DOWN/MOVE 一并让出，
        // 否则它会消费 DOWN（连同 requestDisallowIntercept）把事件流从叠加层手里抢走，
        // 且在叠加层底下启动选区拖动。
        if (!touchEnabled) {
            scaleDetector.onTouchEvent(event)
            gestureDetector.onTouchEvent(event)
            return false
        }
        if (event.action == MotionEvent.ACTION_DOWN) {
            parent?.requestDisallowInterceptTouchEvent(true)
            // 新触摸会停止进行中的惯性动画（标准 Android 可滚动行为；termux 在 onTouchEvent 中亦同）。
            stopFlingAnimation()
        }
        // 抽屉的边缘滑动手势始于屏幕边缘区（~32dp）。Surface 绝不能消费这些触摸：
        // 当 drawerOpen 为假时，Surface 本会收到没有 MOVE 的 DOWN + UP
        // （MOVE 都被抽屉手势取走），GestureDetector 会把它判为轻击而清除选区。
        // 边缘触摸始终属于抽屉手势。抽屉打开时，Surface 必须把全部触摸让给
        // 遮罩（其关闭手势），否则 DOWN 上的 requestDisallowIntercept
        // 会抢走事件流，抽屉将永远无法关闭。
        val drawerEdgePixels = (32 * resources.displayMetrics.density).toInt()
        if (drawerOpen || event.x < drawerEdgePixels) {
            return false
        }
        // 走到这里才说明这次抬手属于终端 Surface（未被抽屉遮罩或边缘手势取走），
        // 此时才播报点击。提前播报会让「轻点遮罩关闭抽屉」「抽屉边缘滑动收尾」
        // 这两条根本不属于终端的抬手被朗读成「已点击」。
        // （滚动与长按拖动的抬手仍在下游被消费，同样会走到这里。）
        if (isRelease) performClick()

        val fromMouse = event.isFromSource(InputDevice.SOURCE_MOUSE)

        if (fromMouse) {
            // 鼠标模式上报（DECSET 1000/1002/1003）：经 Ghostty 鼠标编码器
            // 把鼠标事件路由到终端。应用未启用鼠标上报时编码器返回空序列，
            // 此时事件落到下方的应用手势（右键选词、中键粘贴）。
            val runtime = viewModel?.runtime
            val bridge = runtime?.bridge()
            if (bridge != null) {
                val cellWidth = runtime.cellWidth
                val cellHeight = runtime.cellHeight
                val button =
                    when {
                        event.isButtonPressed(MotionEvent.BUTTON_SECONDARY) -> 1
                        event.isButtonPressed(MotionEvent.BUTTON_TERTIARY) -> 2
                        else -> 0
                    }
                val action =
                    when (event.actionMasked) {
                        MotionEvent.ACTION_UP,
                        MotionEvent.ACTION_CANCEL,
                        -> 1

                        MotionEvent.ACTION_MOVE -> 2

                        else -> 0
                    }
                if (
                    bridge.encodeMouseEvent(
                        event.x,
                        event.y,
                        action,
                        button,
                        KeyModifiers.ghosttyMods(event.metaState),
                        cellWidth,
                        cellHeight,
                    )
                ) {
                    return true
                }
            }

            when {
                event.isButtonPressed(MotionEvent.BUTTON_SECONDARY) -> {
                    if (event.action == MotionEvent.ACTION_DOWN) {
                        viewModel?.clearSelection()
                        startWordSelectionAt(event)
                    }
                    return true
                }

                event.isButtonPressed(MotionEvent.BUTTON_TERTIARY) -> {
                    if (event.action == MotionEvent.ACTION_DOWN) {
                        viewModel?.pasteFromClipboard()
                    }
                    return true
                }
            }
        }

        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                if (isSelectingText) {
                    // 手柄拖动由唯一的覆盖层窗口拥有
                    // （生命周期单一 owner）——落在手柄上的 DOWN 不会再抵达 Surface。
                    // 任何确实抵达此处的 DOWN 都是非手柄轻击：
                    // 与之前一样清除选区。
                    viewModel?.clearSelection()
                    selectionHandles.hideSelectionHandles()
                    hideSelectionMenu("nonHandleTap")
                }
            }

            MotionEvent.ACTION_MOVE -> {
                if (isSelectingText && handleDragState != HandleDrag.NONE) {
                    // 指针 id 锁定：只有锁定拖动的那根手指才能改变选区。
                    // 坐标取自该指针的槽位；由第二根手指携带的移动
                    // ——或它抬起之后的移动——一律吞掉。
                    // 注意：指针 id 与槽位下标是两回事，不能直接比较——
                    // Android 在 ACTION_POINTER_UP 后会回收并复用 id，
                    // 同一手势里两者并不相等。
                    val lockedIdx = dragPointerId?.let { event.findPointerIndex(it) }
                    val lockedMissing = dragPointerId != null && (lockedIdx == null || lockedIdx < 0)
                    if (!lockedMissing) {
                        val touchX = if (lockedIdx != null && lockedIdx >= 0) event.getX(lockedIdx) else event.x
                        val touchY = if (lockedIdx != null && lockedIdx >= 0) event.getY(lockedIdx) else event.y
                        driveHandleDragMove(touchX, touchY)
                    }
                } else if (longPressDragging && isSelectingText) {
                    // 长按自身的抖动不算拖动：越过触摸阈值前一律不动刚选出的词。
                    // 少了这道守卫，1px 位移就把整词选区塌缩成单格（实测
                    // `(1,6)-(1,10)` → `(1,6)-(1,7)`），菜单随选区重锚走位，
                    // 用户点下去的一瞬正好点空——表象即「菜单按钮全部失效」。
                    if (!longPressDragStarted) {
                        val slop = android.view.ViewConfiguration.get(context).scaledTouchSlop
                        val dx = event.x - longPressStartX
                        val dy = event.y - longPressStartY
                        longPressDragStarted = dx * dx + dy * dy > slop * slop
                    }
                    if (longPressDragStarted) {
                        val col = pixelToCell(event.x, cellWidth, cols)
                        val row = pixelToCell(event.y, cellHeight, rows)
                        val gridRow = currentViewportTopGrid() + row
                        viewModel?.updateSelection(gridRow, col)
                        val sel = viewModel?.state?.value?.selection
                        if (sel?.start != null && sel.end != null) {
                            // 重定位而非重建：showSelectionHandles 会 dismiss 并重建
                            // 2 个 PopupWindow（4 次 WindowManager IPC + 分配）
                            // ——在 60-120Hz 的 ACTION_MOVE 下是必然掉帧的来源。
                            // repositionHandle 改用 PopupWindow.update（进程内）。
                            selectionHandles.repositionHandle(HandleDrag.START, sel.start.row, sel.start.col)
                            selectionHandles.repositionHandle(HandleDrag.END, sel.end.row, sel.end.col)
                        }
                    }
                }
            }

            MotionEvent.ACTION_UP,
            MotionEvent.ACTION_CANCEL,
            -> {
                // 手势收尾（慢速拖放无 fling、无 tap 回调时唯一的 settle 点）：
                // 不重置行偏移（保留滚动位置），只结束滚动态并归零余量，
                // 否则残留半行偏移且新输出不再回底（飘移虚浮）。
                if (isScrolling) {
                    isScrolling = false
                    scrollAccumulatorPx = 0f
                    viewModel?.runtime?.setScrollRemainderPx(0f)
                    onScrollingStateChanged?.invoke(false)
                }
                if (longPressDragging) {
                    longPressDragging = false
                    // 长按拖动同样终结一次选区手势：启用轻击保护，
                    // 使松手轻击无法关闭菜单。
                    lastHandleDragEndUptimeMs = SystemClock.uptimeMillis()
                    val sel = viewModel?.state?.value?.selection
                    if (sel?.start != null && sel.end != null) {
                        viewModel?.endSelection()
                        // （termux 对等）：仅粘贴的单格选区同样获得手柄
                        // ——从空白处开始范围选区正是靠拖动它们。
                        // 工具栏经 onGetContentRect 的手柄高度偏移锚定在手柄之上。
                        selectionHandles.showSelectionHandles(
                            sel.start.row,
                            sel.start.col,
                            sel.end.row,
                            sel.end.col,
                            getAccentColor(),
                        )
                        showSelectionMenuForCurrentSelection()
                    }
                }
                if (isSelectingText && handleDragState != HandleDrag.NONE) {
                    finishHandleDrag()
                }
                handleDragState = HandleDrag.NONE
                dragPointerId = null
                invalidateWideCharTailCache()
                try {
                    magnifier?.dismiss()
                } catch (exception: Exception) {
                    LogUtil.w(TAG, "magnifier dismiss failed (non-critical)", exception)
                }
                magnifier = null
                scaleFactor = 1.0f
            }
        }
        return true
    }

    /**
     * 鼠标滚轮事件（外接鼠标/触控板）。应用启用鼠标上报时
     * （DECSET 1000/1002/1003），滚轮事件经 Ghostty 鼠标编码器编码为按钮 4/5
     * 并写入 PTY。未启用上报时编码器返回空序列——事件被忽略
     * （回滚没有滚轮处理，触摸滚动已覆盖该场景）。
     */
    override fun onGenericMotionEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_SCROLL && event.isFromSource(InputDevice.SOURCE_MOUSE)) {
            val runtime = viewModel?.runtime
            val bridge = runtime?.bridge()
            if (bridge != null) {
                val delta = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
                val button = if (delta > 0f) 3 else 4 // wheel-up=3, wheel-down=4 (Rust mapping)
                if (
                    bridge.encodeMouseWheel(
                        event.x,
                        event.y,
                        button,
                        KeyModifiers.ghosttyMods(event.metaState),
                        runtime.cellWidth,
                        runtime.cellHeight,
                        1,
                    )
                ) {
                    return true
                }
            }
        }
        return super.onGenericMotionEvent(event)
    }

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
        applyImeShift()
    }

    // ══════════════════════════════════════════════════════════════════════
    // 五、Surface 生命周期（网格/尺寸由 ResizeManager 拥有）
    // ══════════════════════════════════════════════════════════════════════

    override fun surfaceCreated(holder: SurfaceHolder) {
        // 0 尺寸守卫：布局可能在视图测量完成前就调用 surfaceCreated
        // （宽高为 0）——绑定 0×0 的 ANativeWindow 会创建一个
        // acquireNextImage 失败的黑色交换链（一闪而过）。推迟到 surfaceChanged 拿到真实尺寸。
        if (width <= 0 || height <= 0) return
        val surface = holder.surface
        if (!surface.isValid) return
        // 先认领归属：此后本视图才是唯一有权暂停渲染的那个，旧视图迟到的
        // `surfaceDestroyed` 据此被识别为陈旧（见 `TerminalRuntime.onSurfaceDestroyed`）。
        viewModel?.runtime?.onSurfaceClaimed(surfaceOwnershipToken)
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
                lastConfiguredWidth = width
                lastConfiguredHeight = height
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
        handleDragState = HandleDrag.NONE
        dragPointerId = null
        invalidateWideCharTailCache()
        lastConfiguredWidth = 0
        lastConfiguredHeight = 0
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

// ─────────────────────────────────────────────────────────────────────────────
// 纯辅助函数（顶层，无需视图/bridge 即可单元测试）
// ─────────────────────────────────────────────────────────────────────────────

/**
 * 拖动 y 所在的边缘滚动区：上半格 → 上滚，下半格 → 下滚，中间 → 停止。
 * 与拖动处理器中的非对称边界一致（顶部 `<`、底部 `>=`），
 * 使退化 Surface（高度 < cellHeight）偏向顶部区。
 */
internal enum class EdgeScrollDirection {
    UP,
    DOWN,
    STOP,
}

internal fun edgeScrollDirection(yPx: Float, surfaceHeightPx: Float, cellHeight: Float): EdgeScrollDirection = when {
    yPx < cellHeight / 2 -> EdgeScrollDirection.UP
    yPx >= surfaceHeightPx - cellHeight / 2 -> EdgeScrollDirection.DOWN
    else -> EdgeScrollDirection.STOP
}

/** 像素偏移 → 钳位后的网格单元格（0..maxCells-1；maxCells 为 0 时保持 0）。 */
internal fun pixelToCell(px: Float, cellSize: Float, maxCells: Int): Int = (px / cellSize).toInt().coerceIn(
    0,
    (maxCells - 1).coerceAtLeast(0),
)

/** 纯像素矩形（y 向下增大）：菜单锚定的几何输入，JVM 可单测（不碰 android.jar）。 */
internal data class PixelRect(val left: Int, val top: Int, val right: Int, val bottom: Int)

/**
 * 选区两端点按「先比行、行同再比列」归一化为 (起点, 终点)。
 *
 * 上游保留选择方向，`start`/`end` 可能反向。归一化只有这一处实现：菜单锚定与
 * 取文两处各写一份时曾分叉——取文那份判 `row < row || (row == row && col <= col)`，
 * 菜单那份少了行相等那一支，对向左下的选区会算出 leftCol > rightCol。今天菜单只取
 * `(left + right) / 2`，交换不可见，故未暴露；任何非对称用法（如按宽度裁剪）即继承该错。
 */
internal fun <T> normalizeSelectionAnchors(start: T, end: T, row: (T) -> Int, col: (T) -> Int): Pair<T, T> =
    if (row(start) < row(end) || (row(start) == row(end) && col(start) <= col(end))) {
        start to end
    } else {
        end to start
    }

/**
 * 菜单锚定纯函数（design 决策 3）：返回菜单左上角 (x, y)；无处可放返回 null（调用方隐藏菜单）。规则：
 * - 上方优先：菜单底缘高出选择顶缘一个手柄高（锚隙）；贴顶（越出视口上缘）则翻到选择底缘之下；
 * - 贴右钳制：水平居中于选择后夹进视口左右缘；
 * - 菜单必须整体落在视口内：任一落点越出视口即不可用（选区完全滚出视口时两处都越界）；
 * - 选择盖满视口 —— 上下两处落点都放不进选区外的剩余空间（等价于两处都会与选择相交）
 *   —— 返回 null：菜单任何时刻不遮挡选择。
 */
internal fun menuAnchor(
    selection: PixelRect,
    viewport: PixelRect,
    menuWidth: Int,
    menuHeight: Int,
    handleHeight: Int,
): Pair<Int, Int>? {
    if (viewport.right - viewport.left <= 0 || viewport.bottom - viewport.top <= 0) return null
    val width = menuWidth.coerceAtMost(viewport.right - viewport.left)
    val menuLeft = (
        (selection.left + selection.right) / 2 - width / 2
        ).coerceIn(viewport.left, (viewport.right - width).coerceAtLeast(viewport.left))

    // 两处落点共用同一条「整体在视口内」判据：只判单侧会让选区滚出视口时
    // 返回一个视口外的 y（PopupWindow 被添加到屏幕外，菜单不可见且不报错）。
    fun fits(top: Int) = top >= viewport.top && top + menuHeight <= viewport.bottom
    val aboveTop = selection.top - menuHeight - handleHeight
    if (fits(aboveTop)) {
        return menuLeft to aboveTop
    }
    val belowTop = selection.bottom + handleHeight
    if (fits(belowTop)) {
        return menuLeft to belowTop
    }
    return null
}

/**
 * 滚动到绝对网格行 [row] 时的目标偏移（单位：回滚行数）。
 *
 * 偏移把该行顶推到视口顶，故 `视口顶行 = scrollbackLength - 偏移 = row`；钳到
 * 0..scrollbackLength 使越界的行（低于回滚顶或高于末行）落到能显示的最远处而不
 * 产生负偏移。抽出为纯函数供 JVM 测试——`scrollToRow` 经它计算，测试也经它，
 * 测试里不再自带一份公式（自带时断言恒成立：把 [TerminalSurface.scrollToRow]
 * 整个删掉也判不了红）。
 */
internal fun scrollOffsetForRow(row: Int, scrollbackLength: Int): Int =
    (scrollbackLength - row).coerceIn(0, scrollbackLength)

/**
 * 把绝对回滚网格坐标 (row, col) 映射为视口像素坐标。
 * `viewportTopGrid` 是视口顶部显示的绝对网格行（scrollbackLength - scrollOffset）；
 * 对已是视口相对的行传 0。从内联的 `row - (scrollbackLength - scrollOffset)` 公式中抽出，
 * 使滚动、搜索跳转与字号变化共用一个可测试的换算。
 *
 * 返回未取整的像素；调用方自行取整并钳位到视图边界
 * （手柄锚定在行底部时传入 row + 1）。
 */
internal fun gridToScreen(
    row: Int,
    col: Int,
    viewportTopGrid: Int,
    cellWidth: Float,
    cellHeight: Float,
): Pair<Float, Float> = Pair(col * cellWidth, (row - viewportTopGrid) * cellHeight)

/**
 * 触点列 → 目标列：相对**抓柄时**的参考点做整格位移。
 *
 * 参考点由抓柄时握住的那一侧决定（首端柄 = 首格左缘，末端柄 = 末格右缘），
 * 整次拖动内保持不变。手柄越过另一侧时只换归属，参考点若跟着换，
 * 目标列会在翻转瞬间跳一格：边界反向移动、交叉被立刻撤销，
 * 被拖的柄也会与手指脱开一格（对标 termlib 固定 grabOffset 的做法）。
 */
internal fun dragTargetColumn(
    anchorCol: Int,
    anchorIsStart: Boolean,
    touchX: Float,
    cellWidth: Float,
    cols: Int,
): Int {
    val referenceLocalX = (if (anchorIsStart) anchorCol else anchorCol + 1) * cellWidth
    val deltaCols = ((touchX - referenceLocalX) / cellWidth).roundToInt()
    return (anchorCol + deltaCols).coerceIn(0, (cols - 1).coerceAtLeast(0))
}

/**
 * 触点行 → 目标行（视口行）。
 *
 * 手柄悬于格底，锚点是格底缘，故触点所在行 = 锚点行 + 手指的整行位移。
 * [anchorViewportRow] 是锚点行的视口行号；锚点行与触点 Y 同处视口坐标系，
 * 两者相减即为手指位移，绝对网格行由调用方加上视口顶行。
 */
internal fun dragTargetRow(anchorViewportRow: Int, touchY: Float, cellHeight: Float, rows: Int): Int {
    val referenceLocalY = (anchorViewportRow + 1) * cellHeight
    val deltaRows = ((touchY - referenceLocalY) / cellHeight).roundToInt()
    return (anchorViewportRow + deltaRows).coerceIn(0, (rows - 1).coerceAtLeast(0))
}

/** 组字光标的插入标记矩形（视图局部坐标，平台随后按视图矩阵变换到屏幕）。 */
internal data class InsertionMarker(val horizontal: Float, val top: Float, val baseline: Float, val bottom: Float)

/**
 * 组字光标锚点的标记矩形；光标不在视口内时返回 null（调用方据此不回报）。
 *
 * 另两条准入（输入法正在请求光标监视、当前有组字）在调用前判定：它们只依赖
 * 输入法与组字基线，不需要本次的网格坐标，先判掉就省下那次同步 VT 查询。
 *
 * 取**终端光标格**而非组字文本自身位置，因为组字字节正写进那一格。
 */
internal fun composingCursorMarker(
    cursorRow: Int,
    cursorCol: Int,
    rows: Int,
    cellWidth: Float,
    cellHeight: Float,
): InsertionMarker? {
    if (cursorRow < 0 || cursorRow >= rows || cursorCol < 0) return null
    val (markerX, markerY) = gridToScreen(cursorRow, cursorCol, 0, cellWidth, cellHeight)
    return InsertionMarker(
        horizontal = markerX,
        top = markerY,
        baseline = markerY + cellHeight,
        bottom = markerY + cellHeight,
    )
}

/**
 * 规范化的选区边界：start ≤ end，两个锚点均钳位到网格内。
 * 由 [clampSelection] 产出；供拖动手柄更新路径消费，
 * 使原生 setSelection 绝不看到倒置或越界的单元格。
 */
internal data class SelectionBounds(val startRow: Int, val startCol: Int, val endRow: Int, val endCol: Int)

/**
 * 词选边界解析：取上游 `select_word` 派生的有序界限，失败时退化为落点单格。
 *
 * 两条词选入口（长按、侧键点击）共用本函数，使回退形态不可能分叉：回退分支若只在
 * 其中一条路径上遗漏 `endSelection`，选区会永久停在拖动态，此后输入不再自动贴底。
 */
internal fun resolveWordSelectionBounds(wordBounds: IntArray?, gridRow: Int, col: Int): SelectionBounds =
    if (wordBounds != null && wordBounds.size == SELECTION_BOUNDS_LENGTH) {
        SelectionBounds(wordBounds[0], wordBounds[1], wordBounds[2], wordBounds[3])
    } else {
        SelectionBounds(startRow = gridRow, startCol = col, endRow = gridRow, endCol = col)
    }

/**
 * 保序的范围钳位（termux TextSelectionCursorController.updatePosition 语义）：
 *
 * 1. 把每个锚点钳入 `[0,maxRow] × [0,maxCol]`（max 为负 → 空网格 → 全零）；
 * 2. 输入倒置时交换锚点，使返回的边界始终满足 start ≤ end。
 *
 * 纯函数；支撑 SelectionManager.dragSelection 中拖动路径的纵深防御。
 */
internal fun clampSelection(
    startRow: Int,
    startCol: Int,
    endRow: Int,
    endCol: Int,
    maxRow: Int,
    maxCol: Int,
): SelectionBounds {
    val maxR = maxRow.coerceAtLeast(0)
    val maxC = maxCol.coerceAtLeast(0)
    val sr = startRow.coerceIn(0, maxR)
    val sc = startCol.coerceIn(0, maxC)
    val er = endRow.coerceIn(0, maxR)
    val ec = endCol.coerceIn(0, maxC)
    return if (er < sr || (er == sr && ec < sc)) {
        // 输入倒置：交换以使钳位后 start ≤ end。
        SelectionBounds(startRow = er, startCol = ec, endRow = sr, endCol = sc)
    } else {
        SelectionBounds(startRow = sr, startCol = sc, endRow = er, endCol = ec)
    }
}

/**
 * 单元格列是否为空白（长按仅弹出粘贴菜单；支撑 handleLongPress）。
 *
 * [line] 是 [Bridge.scrollbackLine] 的原样结果：**每列恰好一个字符**，宽字符尾格是
 * 空格占位，故字符下标即列号，直接取 `line[col]` 即可——按宽度表反推列↔字符映射
 * 会在每个宽字符之后整体错位一格（该模型已删除，见归档变更 design 第 1 节）。
 * 宽字符尾格的落点须先经 [snapToWideCharBoundary] 吸附到字符起始列再传入。
 *
 * 空行（null）、空白单元、或行尾之后的任何列都算空白：termux 的
 * `getSelectedText(x,y,x,y)` 对三者都返回 ""，故它们都必须归类为仅粘贴——
 * 原先的 `col < line.length` 合取条件把行尾各列归类为文本，并在那里弹出带
 * PASTE 的完整菜单（根因 A1）。
 */
internal fun isWhitespaceCell(line: String?, col: Int): Boolean = line?.getOrNull(col)?.isWhitespace() != false

/** 手柄拖动结束后的保护窗：期间在松手位置的轻点被吞掉，而不是关闭刚重新显示的菜单（termux 隐藏保护）。 */
internal const val SELECTION_MENU_RESHOW_GUARD_MS = 300L

/**
 * [nowMs] 是否落在上次拖动结束（[lastDragEndMs]，uptimeMillis）后的
 * [SELECTION_MENU_RESHOW_GUARD_MS] 之内。拖动后的首次轻点被视为手势收尾的一部分，
 * 而非「点击选区外 → 关闭」的命令。纯函数；支撑 onSingleTapUp。
 */
internal fun shouldSuppressTapAfterDragEnd(
    nowMs: Long,
    lastDragEndMs: Long,
    windowMs: Long = SELECTION_MENU_RESHOW_GUARD_MS,
): Boolean = lastDragEndMs > 0L && nowMs >= lastDragEndMs && nowMs - lastDragEndMs < windowMs

/** 菜单“打开链接”目标：仅 OSC 8 超链接 URI。纯文本裸 URL 不做识别
 * （libghostty-vt 只提供 OSC 8，不含纯文本 URL 扫描）。显示侧以本函数的
 * 非空性判定，动作与显示绝不分叉。
 */
internal fun resolveOpenLinkUri(hyperlinkUri: String?): String? = hyperlinkUri?.trim()?.takeIf { it.isNotEmpty() }

/** 滚动行高下限：避免除零，保持手势可用。 */
internal const val MIN_CELL_HEIGHT_PX = 1f

/** 滚动一步结果：钳制后的偏移、剩余亚行余量、更新后的位移显出量。 */
internal data class ScrollStep(
    val newOffset: Int,
    val newAccumulatorPx: Float,
    /** 更新后的位移显出量（见 `TerminalSurface.imeShiftRevealPx`）。 */
    val revealPx: Float = 0f,
)

/**
 * 手指滚动增量换算（onScroll 可测核心，同向逻辑）。
 * distanceY 为手势约定（previousY - currentY）：下移为负，进入更早历史（偏移增加）；
 * 上移为正，回到更新内容（偏移减少）。亚行余量累积，整行才移动；边缘钳制并清余量。
 *
 * 边界必须先判，且排在亚行早退之前：已在边界（偏移 0 或回滚长度）继续向边界外拖时，
 * 余量若照常下发，渲染器会把整幅网格按投影亚行平移，边缘露出一条背景空带；等余量
 * 攒够一整行才被清零，内容又整体弹回一行——即「滑到边界跳动闪烁」。故越界位移一律
 * 就地丢弃，不经过渲染器。
 *
 * 回滚区顶端（[scrollbackLength]）还有一层越界：位移把网格顶部裁在屏幕上边界之外，
 * 继续上滑时越界位移喂给 [revealPx]，按像素把那段顶部放回屏幕；反向（回到边界内）
 * 先按同一比例回收显出量再下滚，两个方向都是 1:1。
 *
 * 取整使用截断趋向零（toInt），正/负余量的亚行阈值对称：
 * floor 在负方向过激（-0.9px → -1 行而 +0.9px → 0 行），
 * 导致连续拖动累加 1 行级漂移；toInt 两端均需超过 1 行才触发行变。
 */
internal fun applyScrollDistance(
    accumulatorPx: Float,
    distanceY: Float,
    cellHeightPx: Float,
    scrollOffset: Int,
    scrollbackLength: Int,
    revealPx: Float = 0f,
    revealCapacityPx: Float = 0f,
): ScrollStep {
    var accumulator = accumulatorPx - distanceY
    val cellHeight = cellHeightPx.coerceAtLeast(MIN_CELL_HEIGHT_PX)
    var reveal = revealPx
    if (scrollOffset >= scrollbackLength && accumulator < 0f) {
        // 回到边界内：先 1:1 回收显出量，回收完（余量为 0）才真正下滚。
        val consumed = minOf(-accumulator, reveal)
        reveal -= consumed
        accumulator += consumed
    }
    if (accumulator > 0f && scrollOffset >= scrollbackLength) {
        // 越过回滚区顶端：先按容量显出被位移裁掉的网格顶部，显不出的部分直接丢弃。
        val room = (revealCapacityPx - reveal).coerceAtLeast(0f)
        val revealed = minOf(accumulator, room)
        // 显不出的部分（accumulator - revealed）就地丢弃，不跨行回弹。
        return ScrollStep(scrollOffset, 0f, reveal + revealed)
    }
    if (accumulator < 0f && scrollOffset <= 0) {
        return ScrollStep(scrollOffset, 0f, revealPx = reveal)
    }
    // 截断趋向零：正/负亚行余量对称，消除 floor 非对称导致的 1 行级漂移
    val rawAmount = (accumulator / cellHeight).toInt()
    if (rawAmount == 0) {
        return ScrollStep(scrollOffset, accumulator, revealPx = reveal)
    }
    val newOffset = (scrollOffset + rawAmount).coerceIn(0, scrollbackLength)
    accumulator -= (newOffset - scrollOffset) * cellHeight
    if ((newOffset == 0 && accumulator < 0f) || (newOffset == scrollbackLength && accumulator > 0f)) {
        accumulator = 0f
    }
    return ScrollStep(newOffset, accumulator, revealPx = reveal)
}

/**
 * 惯性模型的时间常数（毫秒）：速度按 `v(t) = v₀·e^(-t/τ)` 衰减，
 * 总行程为 `v₀·τ`（像素）。350ms 的观感与平台样条接近（5000px/s ≈ 1750px 行程，
 * 约 1.5s 内走完 99%），但完全由本仓决定，不受厂商改写影响。
 */
internal const val FLING_TIME_CONSTANT_MS = 350f

/** 惯性动画的时长上限（毫秒）：此后位移增量已不足半像素，继续发 vsync 回调没有意义。 */
internal const val FLING_MAX_DURATION_MS = 2450L

/** 剩余行程小于该值即视为衰减完毕（像素）。 */
internal const val FLING_SETTLE_PX = 0.5f

/**
 * 惯性位移（onFling 可测核心）：给定抬起速度和已过时间，返回**相对起点的行程**（像素）。
 *
 * 纯函数是刻意选的：平台 [android.widget.OverScroller] 会被厂商 ROM 改写，
 * 真机日志里整段样条被"优化"成一帧落点（`force Finished`），一次猛甩直接跳十几行。
 * 指数衰减跨设备一致、单调、有界（上界 `v₀·τ`），且能逐帧落到亚像素，
 * 配合 [applyScrollDistance] 的余量实现逐像素跟随。
 *
 * 单位约定与手势一致：手指上移速度为负，行程也为负（偏移变小、回到更新内容）。
 */
internal fun flingOffsetPx(velocityPxPerSecond: Float, elapsedMs: Long): Float {
    if (elapsedMs <= 0L) return 0f
    val decayed = 1f - exp(-elapsedMs / FLING_TIME_CONSTANT_MS)
    return velocityPxPerSecond * (FLING_TIME_CONSTANT_MS / 1000f) * decayed
}

/** 缩放手势收敛阈值：小于此差值视为回到锚点，只撤销预览不持久化。 */
internal const val ZOOM_FONT_SIZE_EPSILON_SP = 0.05f

/**
 * 缩放手势字号换算（onScale 可测核心）。
 * 基准字号乘以累计缩放因子后钳制到字号上下限。
 *
 * @param spToPxScale sp→像素系数（见 `TerminalRuntime.spToPxScale`），非仅显示密度：
 *   调节条上限与之同源，两者用不同系数会让手势越出调节条允许的区间。
 * @param screenWidthDp 屏幕宽（dp）：上界须与调节条按同一个屏幕推导，否则捏合能捏到
 *   调节条给不出的字号——用户反馈的「设置条与实际可设范围不一致」正是这个分叉。
 */
internal fun zoomFontSize(baseFontSizeSp: Float, scaleFactor: Float, spToPxScale: Float, screenWidthDp: Float): Float =
    (baseFontSizeSp * scaleFactor).coerceIn(
        SettingsRepository.FONT_SIZE_MIN_SP,
        SettingsRepository.fontSizeMaxSp(spToPxScale, screenWidthDp),
    )

/**
 * 缩放手势是否收敛到新字号（onScaleEnd 可测核心）。
 * 终值与基准差值超过阈值才持久化，否则撤销预览恢复基准。
 */
internal fun zoomSettledOnNewSize(baseFontSizeSp: Float, finalSizeSp: Float): Boolean =
    kotlin.math.abs(finalSizeSp - baseFontSizeSp) > ZOOM_FONT_SIZE_EPSILON_SP

/**
 * ACTION_MULTIPLE 的字符载荷。
 *
 * `KeyEvent.getCharacters()` 自 API 29 弃用（官方指向 `commitText`），但硬件键盘的
 * 多字符事件**只**经这条通道送达，而 `InputConnection` 没有接收多字符的非弃用面；
 * 保留并在此定点抑制，处置理由见 `sendKeyEvent` 的 ACTION_MULTIPLE 分支。
 */
@SuppressLint("DeprecatedCall") // 无替代面：多字符事件只经 characters 送达
@Suppress("DEPRECATION") // 同上；Kotlin 编译器告警在 -Werror 下同样需要定点抑制
private fun KeyEvent.multipleCharacters(): String = characters
