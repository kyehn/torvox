package terminal.emulator.ui

import android.content.Context
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
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.widget.Magnifier
import android.widget.OverScroller
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
import terminal.emulator.TouchClass
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.ModifierState
import terminal.emulator.input.applyTerminalEditorInfo
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.InputBatchBuffer
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.computeContentBottomPx
import terminal.emulator.runtime.computeGridAvailableHeight
import terminal.emulator.runtime.computeGridDimensions
import terminal.emulator.runtime.computeImeSurfaceShift
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.util.runCatchingCancellable
import kotlin.math.roundToInt

class TerminalSurface
@JvmOverloads
constructor(context: Context, attrs: AttributeSet? = null, defStyleAttr: Int = 0) :
    SurfaceView(context, attrs, defStyleAttr),
    SurfaceHolder.Callback {

    override fun onDetachedFromWindow() {
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
        cancelSurfaceRecreate()
        // 两个防抖的 resume 回调同样持有本视图：detach 恰在防抖窗内则
        // pause 之后 resume 丢失，渲染永久暂停。直接取消会吞掉配对的 resume
        // ——先把持有的暂停全部归还（计数归零），再取消回调。
        forceResumeRendering()
        // 输入法网格防抖也必须取消，而不只是作废凭证（`forceResumeRendering` 对它
        // 刻意只清凭证，因为它的 `postDelayedSurfaceRecreate` 调用方还要靠 runnable
        // 执行）。detach 场景不同：insets 派发已经停摆、遮挡高度刚被归零，
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
        selectionHandles.hideSelectionHandles()
        hideSelectionMenu("detach")
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
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        // 同一实例可被重新 attach（见 inputBatchBuffer 注释）：重建已关闭的批缓冲。
        inputBatchBuffer = newInputBatchBuffer()
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
     */
    fun showSelectionMenu(pasteOnly: Boolean) {
        hideSelectionMenu("showSelectionMenu")
        if (!isAttachedToWindow) {
            LogUtil.w(TAG, "菜单跳过：未附着")
            return
        }
        val selection = viewModel?.state?.value?.selection
        if (selection == null) {
            LogUtil.w(TAG, "菜单跳过：视图模型为空")
            return
        }
        if (selection.start == null || selection.end == null) {
            LogUtil.w(TAG, "菜单跳过：选区无界")
            return
        }
        val pasteEnabled = clipboardAccess.hasClipboardText()
        val actions = menuActionsForSelection(pasteOnly, pasteEnabled)
        if (actions.isEmpty()) {
            LogUtil.d(TAG, "菜单跳过：无动作")
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
        val anchor = menuAnchor(selection)
        if (anchor == null) {
            LogUtil.d(TAG, "菜单跳过：无处安放")
            return
        }
        val loc = IntArray(2)
        getLocationInWindow(loc)
        try {
            // 本回调只在**我们**调用 dismiss() 时触发。平台直接移除弹窗窗口
            // （父视图窗口令牌变化/销毁）不会走到这里——两者是否成对出现，
            // 是区分「应用主动关菜单」与「平台把窗口收走」的唯一依据。
            popup.setOnDismissListener { LogUtil.d(TAG, "选区菜单 dismiss() 已执行（应用侧主动关闭）") }
            popup.showAtLocation(this@TerminalSurface, 0, loc[0] + anchor.first, loc[1] + anchor.second)
        } catch (exception: Exception) {
            // Activity 在检查与显示之间被 detach——与选区手柄同一守卫；
            // 弹窗从未变为可见。
            LogUtil.w(TAG, "showSelectionMenu: popup show failed", exception)
            return
        }
        selectionMenuPopup = popup
    }

    /** 选区可用菜单项：对标 termux 语义（COPY | SHARE | SELECT ALL | OPEN LINK，剪贴板有文本时提供 PASTE）。 */
    internal fun menuActionsForSelection(pasteOnly: Boolean, pasteEnabled: Boolean): List<Pair<String, () -> Unit>> =
        buildList {
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

    /** 深色胶囊工具栏，每个动作用一个可点击标签承载。 */
    private fun buildMenuBar(actions: List<Pair<String, () -> Unit>>): android.widget.LinearLayout {
        val density = resources.displayMetrics.density
        fun densityPixels(value: Int): Int = (value * density + HALF_PIXEL_OFFSET).toInt()
        val bar =
            android.widget.LinearLayout(context).apply {
                orientation = android.widget.LinearLayout.HORIZONTAL
                val backgroundDrawable = android.graphics.drawable.GradientDrawable()
                backgroundDrawable.setColor(resolveThemeColor(R.attr.colorSurface, R.color.material_color_surface))
                backgroundDrawable.cornerRadius = densityPixels(MENU_BAR_CORNER_RADIUS_DP).toFloat()
                background = backgroundDrawable
                elevation = densityPixels(MENU_ELEVATION_DP).toFloat()
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
                    addView(item)
                }
            }
        return bar
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
        val (topRow, bottomRow) =
            if (start.row <= end.row) start.row to end.row else end.row to start.row
        val (leftCol, rightCol) =
            if (start.row < end.row || start.col <= end.col) {
                start.col to end.col
            } else {
                end.col to start.col
            }
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
     */
    fun hideSelectionMenu(reason: String) {
        val popup = selectionMenuPopup
        selectionMenuPopup = null
        if (popup == null) return
        // 记下消失瞬间的选区状态：菜单的关闭完全由选区状态流驱动，
        // 而 dismiss 的调用点有四条（detach／抓柄／IME 切换／非手柄轻击），
        // 无此日志时菜单意外消失只能靠猜——PasteButtonInstrumentedTest 在
        // CI 同款几何下曾整段丢失粘贴动作而日志里查不到是谁关的。
        val selection = viewModel?.state?.value?.selection
        LogUtil.d(
            TAG,
            "hideSelectionMenu($reason): pasteOnly=${selection?.pasteOnly} " +
                "menuDismissed=${selection?.menuDismissed} touchClass=${selection?.touchClass}",
        )
        try {
            popup.dismiss()
        } catch (exception: Exception) {
            // 已 dismiss——只记日志，使原因不丢失。
            LogUtil.w(TAG, "hideSelectionMenu: dismiss failed", exception)
        }
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
                val reserve =
                    viewModel?.runtime?.let {
                        it.modifierBarHeightPx + it.imeGridReserve()
                    } ?: 0
                val cellWidthPx = lastConfiguredWidth.toFloat() / cols
                val cellHeightPx = (lastConfiguredHeight - reserve).toFloat() / rows
                cols = (width.toFloat() / cellWidthPx).toInt().coerceAtLeast(1)
                rows = ((height - reserve).toFloat() / cellHeightPx).toInt().coerceAtLeast(1)
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
            // 归还（见 [RenderPauseLedger.releaseAndCancel]）；`onSizeChanged` 与
            // `surfaceChanged` 常在同一帧用同一尺寸各调一次本方法，正是这条替换路径的
            // 常见触发。
            val token = pauseLedger.acquire()
            pendingSurfaceResize?.let { removeCallbacks(it) }
            pendingSurfaceResize = null
            pendingSurfaceResizeToken?.let { pauseLedger.releaseAndCancel(it) }
            pendingSurfaceResizeToken = token
            pendingSurfaceResize =
                Runnable {
                    pendingSurfaceResize = null
                    // 以最新尺寸为准：onSizeChanged 已存储了它。
                    applySurfaceResizeNow(surfaceWidthPixels, surfaceHeightPixels)
                    // 稳定触发可能恰好落在已配置的尺寸上而提前返回、来不及执行自身的
                    // 恢复——故始终在此归还。
                    pendingSurfaceResizeToken?.let { pauseLedger.release(it) }
                    pendingSurfaceResizeToken = null
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
                LogUtil.w(TAG, "applySurfaceResize: surface not valid yet, deferring")
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
            forceResumeRendering()
            terminalViewModel.runtime.resumeRendering()
            terminalViewModel.runtime.forceRender()
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
                    private var composingBuffer: String = ""

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

                    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
                        if (isPaused) {
                            composingBuffer = ""
                            return true
                        }
                        // 触摸抑制窗口内不清组字基线、不吞输入：返回 false 让输入法重试，
                        // 否则下次更新以空基线做 diff 致重复文本（N1-24）。
                        if (System.nanoTime() < suppressUntilNanos) return false
                        val newComposing = text?.toString() ?: ""
                        // 纯校对逻辑（ComposingDiff），已单元测试
                        // ——增长/回退/全量重写三种情况集中在一处。
                        val edit = ComposingDiff.reconcile(composingBuffer, newComposing)
                        // 供自动化输入法验证的锚点——日志序列必须与注入的组字文本一一对应。
                        // 门控在调用处而非只靠 LogUtil.d：release 下可省掉整条消息串的拼接。
                        if (terminal.emulator.BuildConfig.DEBUG) {
                            LogUtil.d(
                                "ComposingDiff",
                                "reconcile prev=${composingBuffer.length}ch next=$newComposing " +
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
                        composingBuffer = newComposing
                        return true
                    }

                    override fun finishComposingText(): Boolean {
                        composingBuffer = ""
                        return true
                    }

                    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
                        if (isPaused) {
                            composingBuffer = ""
                            return true
                        }
                        // 同上：抑制窗口内返回 false 让输入法重发该提交，
                        // 返回 true 等于声称已处理，字符永久丢失（N1-24）。
                        if (System.nanoTime() < suppressUntilNanos) return false
                        val committedText = text?.toString() ?: return false
                        val terminalViewModel = viewModel
                        val state = terminalViewModel?.state?.value
                        val ctrlActive =
                            state?.ctrlState == ModifierState.Locked || state?.ctrlState == ModifierState.Once
                        val altActive =
                            state?.altState == ModifierState.Locked || state?.altState == ModifierState.Once

                        if (composingBuffer.isNotEmpty()) {
                            if (committedText == composingBuffer) {
                                // 已经组字增量转发；不再重发。
                            } else {
                                val clear = ComposingDiff.reconcile(composingBuffer, "")
                                sendBackspaces(clear.backspaces)
                                encodeAndSend(committedText, ctrlActive, altActive)
                            }
                            composingBuffer = ""
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
                        return true
                    }

                    override fun sendKeyEvent(event: KeyEvent): Boolean {
                        if (isPaused || System.nanoTime() < suppressUntilNanos) {
                            return true
                        }
                        return if (event.action == KeyEvent.ACTION_DOWN) {
                            handleKeyEvent(event)
                        } else {
                            true
                        }
                    }

                    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                        if (isPaused) return true
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
                        val maxDeletes = composingBuffer.length.coerceAtLeast(MAX_SURROUNDING_DELETES)
                        val safeBefore = beforeLength.coerceIn(0, maxDeletes)
                        val safeAfter = afterLength.coerceIn(0, maxDeletes)
                        if (safeBefore > 0) {
                            // 保持 composingBuffer 与 PTY 将要持有的内容同步：
                            // setComposingText 的增量逻辑（startsWith/追加/退格分支）
                            // 假定该缓冲区镜像「已提交 + 组字」文本。
                            // 否则组字期间的输入法退格会在此删一次，
                            // 又在 setComposingText 的退格分支再删一次，多吃一个字符。
                            if (composingBuffer.isNotEmpty()) {
                                // beforeLength 按码点计数（API 33+）；
                                // 从末尾丢弃同样数量的码点，遍历时跨过代理对，
                                // 使 emoji 与 PTY 内容保持对齐。退格数与移除码点数 1:1。
                                var removed = 0
                                var end = composingBuffer.length
                                while (removed < safeBefore && end > 0) {
                                    val codePoint = composingBuffer.codePointBefore(end)
                                    end -= Character.charCount(codePoint)
                                    removed++
                                }
                                composingBuffer = composingBuffer.substring(0, end)
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
            if (!isAttachedToWindow) return
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
                    setWindowLayoutType(android.view.WindowManager.LayoutParams.TYPE_APPLICATION_SUB_PANEL)
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
        private const val MENU_BAR_CORNER_RADIUS_DP = 8
        private const val MENU_ELEVATION_DP = 6
        private const val MENU_ITEM_TEXT_SIZE_SP = 14f
        private const val MENU_ITEM_HORIZONTAL_PADDING_DP = 16
        private const val MENU_ITEM_VERTICAL_PADDING_DP = 12
        private const val MENU_ESTIMATED_WIDTH_DP = 184
        private const val MENU_HEIGHT_DP = 44
        private const val HALF_PIXEL_OFFSET = 0.5f

        private const val DEFAULT_ROWS = 24
        private const val DEFAULT_COLS = 80
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

        // 单次触摸手势转发的滚轮行数上限：无界 repeat 会在主线程逐行同步
        // 等待原生查询（VT 忙时每行最长 500ms），大幅滑动即冻结 UI。
        // 超限行数由后续手势事件携带新坐标补发——丢弃旧坐标而非阻塞等待。
        private const val MAX_WHEEL_LINES_PER_GESTURE = 8
        private const val SURFACE_RECREATE_RETRY_DELAY_MS = 500L
        private const val SURFACE_RECREATE_ATTEMPTS = 10

        private const val FALLBACK_CELL_WIDTH = 8f
        private const val FALLBACK_CELL_HEIGHT = 16f
        private const val BACKSPACE_BYTE = 0x08.toByte()
        private const val DELETE_BYTE = 0x7F.toByte()

        // deleteSurroundingText 参数的上界（输入法输入不可信）。
        // 4096 足以覆盖「全选删除大块已提交内容」：256 左 > 256 字符的选区
        // （半删除），同时仍限定了 PTY 写入大小。
        private const val MAX_SURROUNDING_DELETES = 4096

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
        viewModel.pasteSink = { sessionId, data -> inputBatchBuffer.write(data, sessionId) }
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

    @Volatile private var rows: Int = DEFAULT_ROWS

    @Volatile private var cols: Int = DEFAULT_COLS
    private var surfaceWidthPixels: Int = 0
    private var surfaceHeightPixels: Int = 0
    private var isScrolling: Boolean = false
    private var scrollAccumulatorPx: Float = 0f

    @Volatile private var scrollOffset: Int = 0
    private var lastImeVisible: Boolean = false

    // 回滚长度缓存：`scrollbackLength()` 是同步 JNI 查询，
    // 当 VT 线程忙于解析大块写入时最多可阻塞 500ms。
    // 手势路径在每个 MotionEvent 上都调用它，
    // 故节流到 ~10Hz 并在其间使用缓存值——避免滚动期间的 UI 线程卡顿/ANR。
    @Volatile private var cachedScrollbackLength: Int = 0

    private var lastScrollbackQueryNanos: Long = 0L

    var touchEnabled: Boolean = true
        set(value) {
            field = value
            isFocusable = value
            isFocusableInTouchMode = value
            if (!value) {
                clearFocus()
            }
        }

    fun setSearchHighlights(data: ByteArray) {
        val bridge = viewModel?.runtime?.bridge() ?: return
        bridge.setSearchHighlights(data.copyOf()) // 为 JNI 防御性拷贝
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
     * 口径与单测一致。五个触发点各自覆盖它的一个输入：输入法遮挡（insets 派发）、
     * 内容下沿、单元格度量、备用屏状态（后三者由 [observeImeShiftInputs] 订阅）、
     * 视口尺寸（[onSizeChanged]，旋转时 Activity 不重建）。
     */
    private fun applyImeShift() {
        val runtime = viewModel?.runtime ?: return
        if (height <= 0) return
        val shift =
            computeImeSurfaceShift(
                contentBottomPx =
                computeContentBottomPx(runtime.lastContentRowFlow.value, runtime.cellHeight),
                surfaceHeightPx = height,
                modifierBarHeightPx = runtime.modifierBarHeightPx,
                imeBottomPx = imeInsetPx,
                isAltScreenActive = runtime.altScreenActiveFlow.value,
            )
        if (shift == appliedImeShiftPx) return
        appliedImeShiftPx = shift
        translationY = -shift.toFloat()
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
        val token = if (runtime.altScreenActiveFlow.value) pauseLedger.acquire() else null
        pendingImeGridResize?.let { removeCallbacks(it) }
        pendingImeGridResize = null
        pendingImeGridResizeToken?.let { pauseLedger.releaseAndCancel(it) }
        pendingImeGridResizeToken = token
        pendingImeGridResize =
            Runnable {
                pendingImeGridResize = null
                resizeManager.applyGridResize(surfaceWidthPixels, surfaceHeightPixels)
                // 凭证已陈旧时（视图 detach 后的 `mRunQueue` 延迟派发）ledger 会把它当
                // 空操作，故这里只清仍属于本次的那一个。
                token?.let { pauseLedger.release(it) }
                if (pendingImeGridResizeToken == token) pendingImeGridResizeToken = null
            }.also { postDelayed(it, IME_RESIZE_DEBOUNCE_MS) }
    }

    /**
     * 两个防抖（交换链重配、输入法网格重排）共用的渲染暂停记账。
     *
     * 运行期的 `setRenderPaused` 是**非计数**布尔（共享的单一 GPU 渲染器），而两个
     * 防抖窗可能重叠：先结束的那一个会把暂停清掉，另一个仍在等稳定尺寸——陈旧缓冲被
     * 拉伸。记账与它的三个不变量见 [RenderPauseLedger]。
     */
    private val pauseLedger =
        RenderPauseLedger { paused -> viewModel?.runtime?.setRenderPaused(paused) }

    /** 在途 [pendingSurfaceResize] 的暂停凭证；取消或执行后 MUST 归还并清空。 */
    private var pendingSurfaceResizeToken: Long? = null

    /** 在途 [pendingImeGridResize] 的暂停凭证（主屏不暂停，故可为空）。 */
    private var pendingImeGridResizeToken: Long? = null

    /**
     * 无条件恢复渲染，并作废两个在途防抖持有的暂停。
     *
     * 用于「我们等的那件事已经发生，不必再等」的入口：Surface 重建成功
     * （[postDelayedSurfaceRecreate]）、交换链重配置完成（[ResizeManager.applyResizeNormal]）、
     * Surface 重新交付（[surfaceCreated]）。这些入口本就必须立刻出一帧（否则黑屏），
     * 此时继续挂着防抖暂停只会让本已就绪的缓冲不显示。
     *
     * 交换链防抖 MUST 一并取消：它等的正是「尺寸稳定」，而本函数的前提就是稳定已达成。
     * 输入法网格防抖 MUST NOT 取消——它的动作（[ResizeManager.applyGridResize]）不碰
     * 交换链，且 [postDelayedSurfaceRecreate] 那条路径后面没有 `applyGridResize` 补做，
     * 取消了就只剩等下一次 insets 变化才自愈；改为只作废它的凭证，让 runnable 照常
     * 执行（陈旧归还已是空操作）。
     */
    private fun forceResumeRendering() {
        pendingSurfaceResize?.let { removeCallbacks(it) }
        pendingSurfaceResize = null
        pendingSurfaceResizeToken = null
        pendingImeGridResizeToken = null
        pauseLedger.reset()
        // 无条件出一帧：本函数的所有调用点都刚拿到可用 Surface。直接写运行期而不走
        // ledger ——ledger 只对自己的持有负责，而这里要覆盖的是别的持有者
        // （切后台等）留下的全局暂停标志。
        viewModel?.runtime?.setRenderPaused(false)
    }

    var onScrollChanged: ((offset: Int) -> Unit)? = null
    var onScrollingStateChanged: ((isScrolling: Boolean) -> Unit)? = null
    var onZoomChanged: ((fontSizeSp: Float) -> Unit)? = null

    // ⑥ 实时缩放预览：双指手势期间每秒数次以插值字号触发，渲染器无需网格 resize 即可跟随。
    // 手势终结时以稳定尺寸调用一次 onZoomChanged（完整应用 + 网格重排）。
    var onZoomPreview: ((fontSizeSp: Float) -> Unit)? = null

    var drawerOpen: Boolean = false
        set(value) {
            field = value
            if (value) {
                selectionHandles.hideSelectionHandles()
            } else {
                // ModalNavigationDrawer 的遮罩轻击会关闭抽屉，
                // 但在关闭动画期间该轻击可能穿透到 TerminalSurface
                // ——终端轻击会清除选区，故关闭抽屉会静默抹掉活跃的文本选区。
                // 在抽屉关闭动画时长（300ms）内抑制「轻击清除」，
                // 使选区能存活抽屉自身的关闭手势。
                suppressUntilNanos = System.nanoTime() + DRAWER_CLOSE_TAP_GRACE_NANOS
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

    @Volatile private var suppressUntilNanos = 0L

    private var pendingUnpauseRunnable: Runnable? = null

    /**
     * 惯性滚动物理（termux TerminalView:1345 模式）：由 [OverScroller] 驱动逐帧减速动画，
     * 而非旧的一次性跳到钳位距离——跳变使惯性滚动显得生硬突兀
     * （「上下滑动…异常并且卡顿」）。[postOnAnimation] 把各步对齐到 vsync；
     * 每步只经 onScrollChanged 推出增量。
     */
    private val flingScroller = OverScroller(context)
    private val flingStepRunnable = Runnable { doFlingStep() }

    /** 把惯性滚动动画推进一个 vsync 节拍。 */
    private fun doFlingStep() {
        if (!flingScroller.computeScrollOffset()) {
            finishFlingAnimation()
            return
        }
        val target = flingScroller.currY.coerceIn(0, currentScrollbackLength())
        if (target != scrollOffset) {
            scrollOffset = target
            onScrollChanged?.invoke(target)
            // 每个惯性步进后按 vsync 节流请求重绘，
            // 使渲染线程（Mailbox）不拖滞地呈现最新帧。
            viewModel?.runtime?.forceRender()
        }
        postOnAnimation(flingStepRunnable)
    }

    /** 停止进行中的惯性动画（触点按下、程序化滚动）。
     *  惯性尚未自然结束时，这也会执行收尾语义——清除亚像素余量并触发
     *  onScrollingStateChanged(false)——使中断后渲染循环的 shouldResetScroll
     *  不会被陈旧的滚动激活标志阻塞。 */
    private fun stopFlingAnimation() {
        if (!flingScroller.isFinished) {
            flingScroller.forceFinished(true)
            removeCallbacks(flingStepRunnable)
            // 惯性中断收尾：与 finishFlingAnimation 同语义，
            // 否则 scrollActive 残留阻塞 render-loop 自动回底。
            scrollAccumulatorPx = 0f
            viewModel?.runtime?.setScrollRemainderPx(0f)
            isScrolling = false
            onScrollingStateChanged?.invoke(false)
        }
    }

    private fun finishFlingAnimation() {
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

    // 拖动锚点：拖动开始时被抓住手柄所固定的单元格边界（网格坐标）。
    // 拖动增量相对该锚点计算，因为手柄窗口悬于其锚定单元格之下
    // ——直接用触摸像素会解析到边界下方的那一行。
    // 锚点语义对标 termux 的 applyHandleDrag（Terminal.kt:1899-1935）；
    // 交叉翻转由 [SelectionState.applyHandleDrag] 实现并经 SelectionStateTest 覆盖
    // （拖动手柄越过静止手柄时归属权交换），此处只负责把触点换算成网格目标。
    private var dragAnchorRow = 0
    private var dragAnchorCol = 0

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
        val anchorLocalY = (dragAnchorRow + 1) * cellHeight
        val deltaRows = ((touchY - anchorLocalY) / cellHeight).roundToInt()
        val row = (dragAnchorRow + deltaRows).coerceIn(0, (rows - 1).coerceAtLeast(0))
        val anchorLocalX =
            if (handleDragState == HandleDrag.START) {
                dragAnchorCol * cellWidth
            } else {
                (dragAnchorCol + 1) * cellWidth
            }
        val deltaCols = ((touchX - anchorLocalX) / cellWidth).roundToInt()
        val col = (dragAnchorCol + deltaCols).coerceIn(0, (cols - 1).coerceAtLeast(0))
        val gridRow = currentViewportTopGrid() + row
        return gridRow to snapToWideCharBoundary(gridRow, col)
    }

    /** 在当前选区范围上重新隐藏并显示选区手柄（拖动结束后使用，使手柄吸附到最终单元格）。 */
    private fun reshowSelectionHandles() {
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
        wideCharDragSession = false
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
                val bounds =
                    viewModel?.dragMove(
                        draggingStart = handleDragState == HandleDrag.START,
                        row = gridRow,
                        col = snapCol,
                        cachedMaxRow = (currentScrollbackLength() + rows - 1).coerceAtLeast(0),
                        cachedMaxCol = (cols - 1).coerceAtLeast(0),
                    )
                if (bounds != null) {
                    selectionHandles.repositionHandle(HandleDrag.START, bounds[0], bounds[1])
                    selectionHandles.repositionHandle(HandleDrag.END, bounds[2], bounds[3])
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
            scrollOffset = newOffset
            onScrollChanged?.invoke(scrollOffset)
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
        val curCol = (currentTouchX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
        val snappedCol = snapToWideCharBoundary(gridRow, curCol)
        if (handleDragState == HandleDrag.START) {
            viewModel?.updateSelectionStart(gridRow, snappedCol)
        } else if (handleDragState == HandleDrag.END) {
            viewModel?.updateSelection(gridRow, snappedCol)
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

    /** 拖动会话进行中：期间只按行号复用缓存，抬手即失效（行内容可能已被新输出改写）。 */
    private var wideCharDragSession = false

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
        wideCharDragSession = true
        // 在拖动开始时只调用一次 setSelectionDragging(true)，
        // 使渲染线程抑制新输出引起的滚动复位。原先在 dragSelection 内
        // 按每个 MOVE 调用——鉴于它是幂等的，那纯属浪费。
        viewModel?.runtime?.setSelectionDragging(true)
        // 抓住仅粘贴选区的手柄会把它升级为文本选区，
        // 使拖动能扩展范围，而不被「仅粘贴不可变」守卫吞掉。
        viewModel?.beginHandleDragOnPasteOnly()
        val selection = viewModel?.state?.value?.selection
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

    private val clipboardAccess = ClipboardAccess(context, tag = "Surface")
    private val resizeManager = ResizeManager()
    private val imeConnection = ImeConnection()
    private val selectionHandles = SelectionHandles()

    val isSelectingText: Boolean
        get() = viewModel?.state?.value?.selection?.active == true

    private val gestureListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(motionEvent: MotionEvent): Boolean {
                // 手势开始时重置亚单元格累加器，使首次 onScroll 距离从干净的起点算起。
                // 同步本地偏移与运行时真源：渲染线程回底后本地仍旧值，下次手势若从旧值起算会跳变。
                viewModel?.runtime?.activeSessionScrollOffset()?.let { scrollOffset = it }
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
                // 备用屏滚轮转发。当远端处于备用屏（vim/less/htop）时，
                // 触摸滚动手势必须以滚轮转义发送给远端而非滚动本地回滚，
                // 否则用户无法在这些程序内部滚动。
                // 每个滚过的行转发一个滚轮事件，与 onGenericMotionEvent
                // 中的外接鼠标路径一致。
                val altBridge = viewModel?.runtime?.bridge()
                if (altBridge != null && altBridge.isAltScreenActive()) {
                    val cellWidth = viewModel?.runtime?.cellWidth ?: 1f
                    val cellHeight = viewModel?.runtime?.cellHeight ?: 1f
                    val pointerXPx = e2.x
                    val pointerYPx = e2.y
                    // 走满一个单元格高度 = 一个滚轮行，与下方的本地滚动映射
                    // （distanceY / cellHeight）一致。
                    val lines = kotlin.math.max(1, kotlin.math.abs((distanceY / cellHeight).toInt()))
                    // 手指上移（distanceY > 0）= 滚轮上（3，较旧）；手指下移 = 滚轮下（4，较新）
                    val button = if (distanceY > 0f) 3 else 4
                    var forwarded = false
                    val cappedLines = lines.coerceAtMost(MAX_WHEEL_LINES_PER_GESTURE)
                    repeat(cappedLines) {
                        val wheelModifiers = KeyModifiers.ghosttyMods(e2.metaState)
                        if (
                            altBridge.encodeMouseEvent(
                                pointerXPx,
                                pointerYPx,
                                0,
                                button,
                                wheelModifiers,
                                cellWidth,
                                cellHeight,
                            )
                        ) {
                            altBridge.encodeMouseEvent(
                                pointerXPx,
                                pointerYPx,
                                1,
                                button,
                                wheelModifiers,
                                cellWidth,
                                cellHeight,
                            )
                            forwarded = true
                        }
                    }
                    return forwarded
                }
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
                    applyScrollDistance(scrollAccumulatorPx, distanceY, cellHeight, scrollOffset, scrollbackLen)
                scrollAccumulatorPx = scrollStep.newAccumulatorPx
                if (scrollStep.newOffset != scrollOffset) {
                    scrollOffset = scrollStep.newOffset
                    onScrollChanged?.invoke(scrollOffset)
                }
                // 逐像素余量：把亚行累加器镜像给渲染器，
                // 使内容在行内跟随手指。
                // 以非空回滚为条件——没有历史时任何偏移都会露出空白。
                // setScrollRemainderPx 已含 notifyRender，不再额外 forceRender
                // （双重唤醒致手势期间渲染线程空转，滚动卡顿）。
                if (scrollbackLen > 0) {
                    viewModel?.runtime?.setScrollRemainderPx(scrollAccumulatorPx)
                }
                return true
            }

            override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
                // 选词中 fling 同样吞掉，避免落到滚动路径致视口跳变。
                if (isSelectingText) return true
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

                // velocityY 系像素/秒，滚动偏移系行：除以行高换算为行/秒，否则 20 倍过速直接撞边，
                // 视口跳变撕裂为“折叠”。fling 前清亚行余量，避免旧余量叠加首帧。
                stopFlingAnimation()
                scrollAccumulatorPx = 0f
                viewModel?.runtime?.setScrollRemainderPx(0f)
                isScrolling = true
                onScrollingStateChanged?.invoke(true)
                // 与 onScroll 同向：velocityY 为像素/秒、下移为正，除以行高得行/秒。
                val rowVelocity = flingRowsPerSecond(velocityY, cellHeight)
                flingScroller.fling(
                    0,
                    scrollOffset,
                    0,
                    rowVelocity,
                    0,
                    0,
                    0,
                    scrollbackLen,
                )
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
                    val spToPxScale = viewModel?.runtime?.spToPxScale ?: return false
                    val sizeSp = zoomFontSize(zoomBaseFontSizeSp, scaleFactor, spToPxScale)
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
                    val spToPxScale = viewModel?.runtime?.spToPxScale ?: return
                    // 落定判定用未钳位值：钳位会把无缩放手势（8sp 基准×1.0→钳制 14sp）
                    // 误判为新尺寸并持久化，未钳位比较只在真实缩放时落定。
                    val rawSizeSp = zoomBaseFontSizeSp * scaleFactor
                    val sizeSp = zoomFontSize(zoomBaseFontSizeSp, scaleFactor, spToPxScale)
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
            viewModel?.startSelection(gridRow, col, TouchClass.Whitespace)
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
            val wordBounds = bridge?.selectWordAt(gridRow, col)

            val startRow: Int
            val startCol: Int
            val endRow: Int
            val endCol: Int

            if (wordBounds != null && wordBounds.size == SELECTION_BOUNDS_LENGTH) {
                startRow = wordBounds[0]
                startCol = wordBounds[1]
                endRow = wordBounds[2]
                endCol = wordBounds[3]
            } else {
                // 无可选词/查询失败：退化为落点单格（网格坐标），
                // 粘贴菜单语义不变。
                startRow = gridRow
                startCol = col
                endRow = gridRow
                endCol = col
            }

            LogUtil.d(
                "Selection",
                "LONG_PRESS text: tapRow=$row tapCol=$col gridRow=$gridRow " +
                    "expanded start=($startRow,$startCol) end=($endRow,$endCol) " +
                    "menu=FULL cellW=$cellWidth cellH=$cellHeight rows=$rows cols=$cols",
            )

            viewModel?.startSelection(startRow, startCol, TouchClass.Text)
            viewModel?.updateSelection(endRow, endCol)
            viewModel?.endSelection()
            selectionHandles.showSelectionHandles(startRow, startCol, endRow, endCol, getAccentColor())
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
            isPaused = true
            imeConnection.currentInputConnection?.let { ic ->
                ic.finishComposingText()
            }
            suppressUntilNanos = System.nanoTime() + SUPPRESS_GRACE_PERIOD_NS
        }
        if (hasFocus) {
            // 重置暂停状态，但绝不要调用 finishComposingText——输入法可能正处于
            // 组字中；在此中止会导致输入法失步，产生重复文本、多余空格或静默丢字。
            isPaused = false
            suppressUntilNanos = System.nanoTime() + SUPPRESS_GRACE_PERIOD_NS
        }
    }

    /** 在途的 surface 重建重试；detach 与 ON_PAUSE 必须能取消它（见 [postDelayedSurfaceRecreate]）。 */
    private var pendingSurfaceRecreate: Runnable? = null

    /** 取消在途的 surface 重建重试。宿主切后台时必须调用，否则重试会撤销刚请求的渲染暂停。 */
    fun cancelSurfaceRecreate() {
        pendingSurfaceRecreate?.let { removeCallbacks(it) }
        pendingSurfaceRecreate = null
    }

    fun postDelayedUnpause(delayMillis: Long) {
        pendingUnpauseRunnable?.let { removeCallbacks(it) }
        pendingUnpauseRunnable =
            Runnable {
                pendingUnpauseRunnable = null
                if (hasWindowFocus()) {
                    isPaused = false
                }
            }
                .also { postDelayed(it, delayMillis) }
    }

    /**
     * 切应用恢复：ON_RESUME 时 holder 的 Surface 往往仍无效
     * （系统在后台回收了 BufferQueue，且未送达 surfaceDestroyed）。
     * 持续重试 detach+attach 的交换链重建直到 holder 重新有效，
     * 然后解除暂停 + 恢复 + 强制渲染一帧。
     *
     * 重试链必须挂在字段上才能被取消：它每次尝试都会 `forceResumeRendering()`
     * （内含 `setRenderPaused(false)`）+ `resumeRendering()`，一旦跨越 ON_PAUSE 就会撤销宿主刚请求的暂停，
     * 在已被回收的 BufferQueue 上继续渲染。且每次重试都是新 lambda，
     * 即便有字段也要由本函数自己重排。
     */
    fun postDelayedSurfaceRecreate(viewModel: TerminalViewModel, attemptsLeft: Int = SURFACE_RECREATE_ATTEMPTS) {
        pendingSurfaceRecreate?.let { removeCallbacks(it) }
        pendingSurfaceRecreate =
            Runnable {
                pendingSurfaceRecreate = null
                val holderSurface = holder?.surface
                if (holderSurface != null && holderSurface.isValid && width > 0 && height > 0) {
                    val bridge = viewModel.runtime.bridge()
                    if (bridge != null) {
                        viewModel.currentSurface = holderSurface
                        bridge.releaseGpuSurface()
                        bridge.attachSurface(holderSurface, width, height)
                    }
                    forceResumeRendering()
                    viewModel.runtime.resumeRendering()
                    viewModel.runtime.forceRender()
                } else if (attemptsLeft > 1) {
                    postDelayedSurfaceRecreate(viewModel, attemptsLeft - 1)
                }
            }.also { postDelayed(it, SURFACE_RECREATE_RETRY_DELAY_MS) }
    }

    private fun currentScrollbackLength(): Int {
        val now = System.nanoTime()
        if (now - lastScrollbackQueryNanos < SCROLLBACK_QUERY_THROTTLE_NANOS) {
            return cachedScrollbackLength
        }
        val viewModel = viewModel ?: return cachedScrollbackLength
        val bridge = viewModel.runtime.bridge() ?: return cachedScrollbackLength
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
        val targetOffset = (scrollbackLen - row).coerceIn(0, scrollbackLen)
        if (targetOffset != scrollOffset) {
            scrollOffset = targetOffset
            onScrollChanged?.invoke(scrollOffset)
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
        }
    }

    fun getScrollOffset(): Int {
        // 运行时为真源：新输出在渲染线程将 entry 置 0，若只读本地字段则回车后视图永不回底。
        // 手势期间本地与运行时同步更新（onScroll 双写），此处优先读运行时保证回底可见。
        return viewModel?.runtime?.activeSessionScrollOffset() ?: scrollOffset
    }

    fun getMaxScrollOffset(): Int = currentScrollbackLength()

    fun getRows(): Int = rows

    fun getCols(): Int = cols

    // 视图 detach 时 close() 会 shutdown 发送线程；同一实例再次 attach（窗口转换、
    // 分屏、Compose 重新挂载）时必须重建，否则每次 write 都抛 RejectedExecutionException
    // ——输入法输入与粘贴会在无任何症状的情况下永久失效。
    private var inputBatchBuffer = newInputBatchBuffer()

    private fun newInputBatchBuffer(): InputBatchBuffer = InputBatchBuffer(
        flushSink = { sessionId, data -> viewModel?.writeToPty(sessionId, data) },
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
        // 上游 select_word 词界 — 与长按完全同一 native 派生，无两侧分叉。
        val bridge = viewModel?.runtime?.bridge()
        val gridRow = currentViewportTopGrid() + row
        val wordBounds = bridge?.selectWordAt(gridRow, col)
        if (wordBounds != null && wordBounds.size == SELECTION_BOUNDS_LENGTH) {
            viewModel?.startSelection(wordBounds[0], wordBounds[1])
            viewModel?.updateSelection(wordBounds[2], wordBounds[3])
            viewModel?.endSelection()
            LogUtil.d(
                "Selection",
                "RIGHT_CLICK word: tapRow=$row tapCol=$col " +
                    "expanded start=(${wordBounds[0]},${wordBounds[1]}) " +
                    "end=(${wordBounds[2]},${wordBounds[3]})",
            )
        } else {
            // 无可选词：单格回退。选区状态用网格行（0 = 回滚顶部），
            // 此处已换算为 gridRow，抽取与控柄渲染一致。
            viewModel?.startSelection(gridRow, col)
        }

        try {
            magnifier = magnifier ?: Magnifier.Builder(this@TerminalSurface).build()
            magnifier?.show(event.rawX, event.rawY)
        } catch (exception: Exception) {
            LogUtil.w(TAG, "magnifier show failed (non-critical)", exception)
        }
    }

    private fun modifierBitmask(event: KeyEvent): Byte {
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
            val success = bridge.processKeyEvent(keyCode, modifiers, action, event.unicodeChar)
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
                wideCharDragSession = false
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
                val mods = KeyModifiers.ghosttyMods(event.metaState)
                if (
                    bridge.encodeMouseEvent(
                        event.x,
                        event.y,
                        0,
                        button,
                        mods,
                        runtime.cellWidth,
                        runtime.cellHeight,
                    )
                ) {
                    // 滚轮释放完成滚动手势。
                    bridge.encodeMouseEvent(
                        event.x,
                        event.y,
                        1,
                        button,
                        mods,
                        runtime.cellWidth,
                        runtime.cellHeight,
                    )
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
                // onSurfaceDestroyed 会在此路径上置 render_paused=true，
                // 而只有设置界面会清除它，
                // 故普通的后台/恢复循环会让该标志保持置位，
                // 使重启的渲染线程输出黑帧（暂停时 render_frame 会短路）。
                // 在线程重启前清除它。
                forceResumeRendering()
                terminalViewModel.runtime.resumeRendering()
                terminalViewModel.runtime.forceRender()
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
        viewModel?.runtime?.onSurfaceDestroyed()
        viewModel?.runtime?.releaseAllGpuSurfaces()
        // 关闭选区/光标手柄弹窗：它们持有 Activity 上下文，
        // 而「已 dismiss 但仍显示」的弹窗会触发 StrictMode 的
        // Activity 泄漏（在 Activity 销毁期间旋转时还会触发 BadTokenException 崩溃）。
        selectionHandles.hideSelectionHandles()
        // 重置因 Surface 拆除而中断的拖动所残留的任何锁定：
        // 覆盖层已消失故不会再有 UP 到达；陈旧的锁定只能靠下一次触摸自愈。
        handleDragState = HandleDrag.NONE
        dragPointerId = null
        wideCharDragSession = false
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
        viewModel?.runtime?.runAfterRenderThreadsStopped {
            viewModel?.currentSurface = null
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
 * 规范化的选区边界：start ≤ end，两个锚点均钳位到网格内。
 * 由 [clampSelection] 产出；供拖动手柄更新路径消费，
 * 使原生 setSelection 绝不看到倒置或越界的单元格。
 */
internal data class SelectionBounds(val startRow: Int, val startCol: Int, val endRow: Int, val endCol: Int)

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

/** 滚动一步结果：钳制后的偏移与剩余亚行余量。 */
internal data class ScrollStep(val newOffset: Int, val newAccumulatorPx: Float)

/**
 * 手指滚动增量换算（onScroll 可测核心，同向逻辑）。
 * distanceY 为手势约定（previousY - currentY）：下移为负，进入更早历史（偏移增加）；
 * 上移为正，回到更新内容（偏移减少）。亚行余量累积，整行才移动；边缘钳制并清余量。
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
): ScrollStep {
    var accumulator = accumulatorPx - distanceY
    val cellHeight = cellHeightPx.coerceAtLeast(MIN_CELL_HEIGHT_PX)
    // 截断趋向零：正/负亚行余量对称，消除 floor 非对称导致的 1 行级漂移
    val rawAmount = (accumulator / cellHeight).toInt()
    if (rawAmount == 0) {
        return ScrollStep(scrollOffset, accumulator)
    }
    val newOffset = (scrollOffset + rawAmount).coerceIn(0, scrollbackLength)
    accumulator -= (newOffset - scrollOffset) * cellHeight
    if ((newOffset == 0 && accumulator < 0f) || (newOffset == scrollbackLength && accumulator > 0f)) {
        accumulator = 0f
    }
    return ScrollStep(newOffset, accumulator)
}

/**
 * 惯性行速度换算（onFling 可测核心，与拖动同向）。
 * velocityY 为手势约定（像素/秒，下移为正）：下移进入更早历史（正行速度），
 * 上移回到更新内容（负行速度）。行高归一避免除零过速撞边。
 */
internal fun flingRowsPerSecond(velocityYPxPerSecond: Float, cellHeightPx: Float): Int =
    (velocityYPxPerSecond / cellHeightPx.coerceAtLeast(MIN_CELL_HEIGHT_PX)).toInt()

/** 缩放手势收敛阈值：小于此差值视为回到锚点，只撤销预览不持久化。 */
internal const val ZOOM_FONT_SIZE_EPSILON_SP = 0.05f

/**
 * 缩放手势字号换算（onScale 可测核心）。
 * 基准字号乘以累计缩放因子后钳制到字号上下限。
 *
 * @param spToPxScale sp→像素系数（见 `TerminalRuntime.spToPxScale`），非仅显示密度：
 *   调节条上限与之同源，两者用不同系数会让手势越出调节条允许的区间。
 */
internal fun zoomFontSize(baseFontSizeSp: Float, scaleFactor: Float, spToPxScale: Float): Float =
    (baseFontSizeSp * scaleFactor).coerceIn(
        SettingsRepository.FONT_SIZE_MIN_SP,
        SettingsRepository.fontSizeMaxSp(spToPxScale),
    )

/**
 * 缩放手势是否收敛到新字号（onScaleEnd 可测核心）。
 * 终值与基准差值超过阈值才持久化，否则撤销预览恢复基准。
 */
internal fun zoomSettledOnNewSize(baseFontSizeSp: Float, finalSizeSp: Float): Boolean =
    kotlin.math.abs(finalSizeSp - baseFontSizeSp) > ZOOM_FONT_SIZE_EPSILON_SP
