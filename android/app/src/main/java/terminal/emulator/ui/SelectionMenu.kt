package terminal.emulator.ui

import android.content.Intent
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.PopupWindow
import android.widget.TextView
import androidx.core.content.ContextCompat
import androidx.core.net.toUri
import terminal.emulator.R
import terminal.emulator.SelectionState
import terminal.emulator.runtime.LogUtil
import java.util.Locale

private const val MENU_BAR_CORNER_RADIUS_DP = 8
private const val MENU_ELEVATION_DP = 6
private const val MENU_ITEM_TEXT_SIZE_SP = 14f
private const val MENU_ITEM_HORIZONTAL_PADDING_DP = 16
private const val MENU_ITEM_VERTICAL_PADDING_DP = 12
private const val MENU_ESTIMATED_WIDTH_DP = 184
private const val MENU_HEIGHT_DP = 44
private const val HALF_PIXEL_OFFSET = 0.5f

/**
 * 拥有选区上下文菜单：以 [PopupWindow] 实现，与选区手柄相同的独立系统窗口模式。
 *
 * 菜单的创建、原地更新与关闭全在本类内；[TerminalSurface] 只转发外部事件。
 */
internal class SelectionMenu(private val surface: TerminalSurface) {
    private var selectionMenuPopup: PopupWindow? = null

    /** 菜单条视图，与 [selectionMenuPopup] 同生命周期（原地换条目时复用，见 [showSelectionMenu]）。 */
    private var selectionMenuBar: LinearLayout? = null

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
        val selection = surface.viewModel?.state?.value?.selection
        if (selection == null) {
            LogUtil.warn(TerminalSurface.TAG, "菜单跳过：视图模型为空")
            return
        }
        if (selection.start == null || selection.end == null) {
            LogUtil.warn(TerminalSurface.TAG, "菜单跳过：选区无界")
            return
        }
        // 文本区动作的显示判定与动作解析共用同一份文本（viewModel.selectionTextForAction
        // → SelectionManager.resolvedSelectionText）。空文本的选区不再挂出「复制/分享」：
        // 点下去只会静默返回，用户看到的是「按钮无效」，而这与「按不动」无法区分。
        val hasCopyableText = surface.viewModel?.selectionTextForAction()?.isNotEmpty() == true
        val actions =
            menuActionsForSelection(pasteOnly, surface.clipboardAccess.hasClipboardText(), hasCopyableText)
        if (actions.isEmpty()) {
            LogUtil.debug(TerminalSurface.TAG, "菜单跳过：无动作")
            return
        }
        val anchor = menuAnchor(selection)
        if (anchor == null) {
            LogUtil.debug(TerminalSurface.TAG, "菜单跳过：无处安放")
            return
        }
        val windowLocation = IntArray(2)
        surface.getLocationInWindow(windowLocation)
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
                windowLocation[0] + anchor.first,
                windowLocation[1] + anchor.second,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            )
            LogUtil.debug(
                TerminalSurface.TAG,
                "菜单原地更新：at=(${windowLocation[0] + anchor.first},${windowLocation[1] + anchor.second}) labels=$labels",
            )
            return
        }
        // 旧窗口已不在（平台移除或已 dismiss）：清掉引用再重建。
        hideSelectionMenu("showSelectionMenu", restoreHandles = false)
        if (surface.viewDetached || !surface.isAttachedToWindow) {
            // 视图未附着时的正常退化（平台正在拆除或已重建）：按 debug 记，
            // 不占 W 闸门（见 `logcat-hygiene`「应用自身的正常路径不得产生警告与错误」）。
            LogUtil.debug(TerminalSurface.TAG, "菜单跳过：未附着")
            return
        }
        val menuBar = buildMenuBar(actions)
        val popup =
            PopupWindow(
                menuBar,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            )
                .apply {
                    isOutsideTouchable = false
                    isFocusable = false
                    setBackgroundDrawable(null)
                    setAnimationStyle(0)
                    setWindowLayoutType(
                        WindowManager.LayoutParams.TYPE_APPLICATION_SUB_PANEL,
                    )
                }
        try {
            // 本回调只在**我们**调用 dismiss() 时触发。平台直接移除弹窗窗口
            // （父视图窗口令牌变化/销毁）不会走到这里——两者是否成对出现，
            // 是区分「应用主动关菜单」与「平台把窗口收走」的唯一依据。
            popup.setOnDismissListener {
                LogUtil.debug(TerminalSurface.TAG, "选区菜单 dismiss() 已执行（应用侧主动关闭）")
            }
            popup.showAtLocation(
                surface,
                0,
                windowLocation[0] + anchor.first,
                windowLocation[1] + anchor.second,
            )
            LogUtil.debug(
                TerminalSurface.TAG,
                "菜单显示：at=(${windowLocation[0] + anchor.first},${windowLocation[1] + anchor.second}) " +
                    "pasteOnly=$pasteOnly actions=${actions.size}",
            )
        } catch (exception: Exception) {
            // Activity 在检查与显示之间被 detach——与选区手柄同一守卫；
            // 弹窗从未变为可见。
            LogUtil.warn(TerminalSurface.TAG, "showSelectionMenu: popup show failed", exception)
            return
        }
        selectionMenuPopup = popup
        selectionMenuBar = menuBar
        shownMenuLabels = actions.map { it.first }
    }

    /** 选区可用菜单项：对标 termux 语义（COPY | SHARE | SELECT ALL | OPEN LINK，剪贴板有文本时提供 PASTE）。
     *
     * [hasCopyableText] 由调用方用 [terminal.emulator.TerminalViewModel.selectionTextForAction]（即动作真正
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
                    surface.context.getString(R.string.paste) to
                        {
                            surface.viewModel?.pasteFromClipboard()
                            surface.viewModel?.clearSelection()
                        },
                )
            }
        } else {
            if (hasCopyableText) {
                add(
                    surface.context.getString(R.string.copy) to
                        {
                            surface.viewModel?.copySelectionToClipboard()
                            surface.viewModel?.clearSelection()
                        },
                )
                add(
                    surface.context.getString(R.string.share) to
                        {
                            surface.viewModel?.shareSelection()
                            surface.viewModel?.clearSelection()
                        },
                )
            }
            add(
                surface.context.getString(R.string.select_all) to
                    {
                        // termux 行为：全选后保持菜单打开，用户可立即复制新选区。
                        surface.viewModel?.selectAll()
                        // 选择几何剧变：隐藏→按新界限重显完成重锚（design 决策 3）。
                        showSelectionMenuForCurrentSelection()
                    },
            )
            // 打开链接项：仅 OSC 8 超链接（libghostty-vt 不识别纯文本裸 URL）。
            // 显示判定与动作解析共用同一输入，绝不出现“项显示却无目标”。
            val selectionHyperlinkUri = selectionHyperlinkUri()
            if (selectionHyperlinkUri != null) {
                add(
                    surface.context.getString(R.string.open_link) to
                        {
                            openSelectionAsLink(selectionHyperlinkUri)
                            surface.viewModel?.clearSelection()
                        },
                )
            }
        }
    }

    /** 选区起点的 OSC 8 超链接 URI，无链接返回 null。 */
    private fun selectionHyperlinkUri(): String? = surface.viewModel
        ?.state
        ?.value
        ?.selection
        ?.start
        ?.let { surface.viewModel?.runtime?.bridge()?.hyperlinkAt(it.row, it.col) }
        ?.let { resolveOpenLinkUri(it) }

    /** 菜单“打开链接”动作：http(s) 白名单后交系统打开；无目标静默返回。 */
    internal fun openSelectionAsLink(hyperlinkUri: String) {
        val target = resolveOpenLinkUri(hyperlinkUri) ?: return
        val uri = try {
            target.toUri()
        } catch (_: IllegalArgumentException) {
            return
        }
        val scheme = uri.scheme?.lowercase(Locale.ROOT)
        if (scheme != "http" && scheme != "https") return
        try {
            val intent =
                Intent(Intent.ACTION_VIEW, uri)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            surface.context.startActivity(intent)
        } catch (exception: Exception) {
            LogUtil.warn(TerminalSurface.TAG, "openSelectionAsLink: no handler", exception)
        }
    }

    /**
     * 深色胶囊工具栏，每个动作用一个可点击标签承载（创建窗口时用；
     * 已在屏幕上的菜单原地换条目走 [fillMenuBar]）。
     */
    private fun buildMenuBar(actions: List<Pair<String, () -> Unit>>): LinearLayout {
        val menuBar = LinearLayout(surface.context)
        fillMenuBar(menuBar, actions)
        return menuBar
    }

    /** 把 [actions] 装进 [menuBar]：先清空再重建按钮，窗口与背景复用（见 [showSelectionMenu]）。 */
    private fun fillMenuBar(menuBar: LinearLayout, actions: List<Pair<String, () -> Unit>>) {
        val density = surface.resources.displayMetrics.density
        fun densityPixels(value: Int): Int = (value * density + HALF_PIXEL_OFFSET).toInt()
        menuBar.orientation = LinearLayout.HORIZONTAL
        val backgroundDrawable = GradientDrawable()
        backgroundDrawable.setColor(resolveThemeColor(R.attr.colorSurface, R.color.material_color_surface))
        backgroundDrawable.cornerRadius = densityPixels(MENU_BAR_CORNER_RADIUS_DP).toFloat()
        menuBar.background = backgroundDrawable
        menuBar.elevation = densityPixels(MENU_ELEVATION_DP).toFloat()
        menuBar.removeAllViews()
        for ((label, action) in actions) {
            val item =
                TextView(surface.context).apply {
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
            menuBar.addView(item)
        }
    }

    /**
     * 解析 Material 3 主题颜色属性（spec：选择菜单样式 MUST 用 `colorSurface`/
     * `colorOnSurface` 主题属性、MUST NOT 硬编码颜色）；属性未定义时回退同值的
     * 颜色资源，Kotlin 侧不出现任何字面颜色值。
     */
    private fun resolveThemeColor(attribute: Int, fallbackColorResource: Int): Int {
        val typedValue = TypedValue()
        if (!surface.context.theme.resolveAttribute(attribute, typedValue, true)) {
            return ContextCompat.getColor(surface.context, fallbackColorResource)
        }
        return if (typedValue.resourceId != 0) {
            ContextCompat.getColor(surface.context, typedValue.resourceId)
        } else {
            typedValue.data
        }
    }

    /**
     * 选择几何（网格 → 视图像素）→ 纯函数 [menuAnchor] 的输入：选择矩形、视口
     * 与菜单估计尺寸。返回 null（无处可放/度量未就绪）时调用方隐藏菜单。
     */
    private fun menuAnchor(selection: SelectionState): Pair<Int, Int>? {
        val cellWidthPixels = surface.cellWidth
        val cellHeightPixels = surface.cellHeight
        if (cellWidthPixels <= 0f || cellHeightPixels <= 0f) return null
        val start = selection.start ?: return null
        val end = selection.end ?: return null
        // 行按先后归一（与取文共用 `normalizeSelectionAnchors`）；列取包围盒极值——
        // 多行选区的列是各行内偏移，不存在「全局左右」，故不能跟着行一起换。
        val (lowerAnchor, upperAnchor) = normalizeSelectionAnchors(start, end, { it.row }, { it.col })
        val topRow = lowerAnchor.row
        val bottomRow = upperAnchor.row
        val leftCol = minOf(start.col, end.col)
        val rightCol = maxOf(start.col, end.col)
        val viewportTopGrid = surface.currentViewportTopGrid()
        val (leftPx, topPx) = gridToScreen(topRow, leftCol, viewportTopGrid, cellWidthPixels, cellHeightPixels)
        val (rightPx, bottomPx) =
            gridToScreen(bottomRow + 1, rightCol + 1, viewportTopGrid, cellWidthPixels, cellHeightPixels)
        val density = surface.resources.displayMetrics.density
        val densityPixels = { value: Int -> (value * density + HALF_PIXEL_OFFSET).toInt() }
        // PopupWindow 在显示时才测量，故用固定估算宽度（[MENU_ESTIMATED_WIDTH_DP]，
        // 覆盖选区菜单的项数上限）并钳位到 Surface。
        val estimatedWidth = surface.width.coerceAtMost(densityPixels(MENU_ESTIMATED_WIDTH_DP))
        val menuHeight = densityPixels(MENU_HEIGHT_DP)
        return menuAnchor(
            selection = PixelRect(leftPx.toInt(), topPx.toInt(), rightPx.toInt(), bottomPx.toInt()),
            viewport = PixelRect(0, 0, surface.width, surface.height),
            menuWidth = estimatedWidth,
            menuHeight = menuHeight,
            handleHeight = selectionHandleHeight().toInt(),
        )
    }

    /** 选择手柄高度（与手柄定位同源；未知时回退一 Character 行高）。 */
    private fun selectionHandleHeight(): Float {
        val content = surface.selectionHandles.contentHandleHeight()
        return if (content > 0) content.toFloat() else surface.cellHeight
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
        val selection = surface.viewModel?.state?.value?.selection
        LogUtil.debug(
            TerminalSurface.TAG,
            "hideSelectionMenu($reason): pasteOnly=${selection?.pasteOnly} " +
                "menuDismissed=${selection?.menuDismissed}",
        )
        try {
            popup.dismiss()
        } catch (exception: Exception) {
            // 已 dismiss——只记日志，使原因不丢失。
            LogUtil.warn(TerminalSurface.TAG, "hideSelectionMenu: dismiss failed", exception)
        }
        if (restoreHandles) surface.reshowSelectionHandles()
    }

    /** 手势完成点同步亮出当前选择的菜单（控制柄同模式），Compose 侧作为同步备份。 */
    internal fun showSelectionMenuForCurrentSelection() {
        val selection = surface.viewModel?.state?.value?.selection
        if (selection == null) {
            LogUtil.debug(TerminalSurface.TAG, "同步备份跳过：视图模型为空")
            return
        }
        if (!selection.active || selection.start == null || selection.end == null) {
            LogUtil.debug(TerminalSurface.TAG, "同步备份跳过：选区未就绪")
            return
        }
        showSelectionMenu(selection.pasteOnly)
    }
}
