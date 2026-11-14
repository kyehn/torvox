package terminal.emulator.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Rect
import android.graphics.drawable.Drawable
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.PopupWindow
import terminal.emulator.runtime.LogUtil
import android.R as AndroidR

/** 手柄拖动归属：起点柄、终点柄或无拖动。 */
internal enum class HandleDrag {
    NONE,
    START,
    END,
}

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
 * 拖动状态（handleDragState/HandleDrag/dragPointerId）保留在终端 Surface——
 * 触摸路径与边缘滚动 runnable 会读取它们。
 */
internal class SelectionHandles(private val surface: TerminalSurface) {
    private var overlayPopup: PopupWindow? = null
    private var overlayContent: HandleOverlayLayout? = null
    private val startHandleRect = Rect()
    private val endHandleRect = Rect()

    /**
     * 覆盖层窗口的存活追踪（见 [PopupWindowLiveness]）。
     *
     * 判定窗口存活不能只看 `contentView.isAttachedToWindow`：同一次选区的输入路径与
     * Compose 副作用会在同一帧里各调一次 [showSelectionHandles]，第二次调用时窗口
     * 已 `showAtLocation`、但还没走完首帧遍历，`isAttachedToWindow` 仍为 false。
     * 若据此拆窗重建，一帧内会创建并丢弃多个 PopupWindow——每个被丢弃的窗口都留下
     * 一个从未在 InputManager 注册的输入通道，GC 时打出
     * `InputManager-JNI: Input channel object 'PopupWindow:...' was disposed without
     * first being removed with the input manager!`。
     */
    private var overlayLiveness: PopupWindowLiveness? = null
    private var overlayWindowX = 0
    private var overlayWindowY = 0

    /** 手柄固有宽度（px），由平台主题 drawable 量出；命中矩形按它计算。 */
    private var selectionHandleWidth = 0

    /**
     * 系统 Material 选区手柄：解析平台主题属性
     * （AndroidR.attr.textSelectHandleLeft/Right），
     * 使手柄形态是框架自带的泪滴形而非自定义矢量图。
     */
    internal fun resolveSelectionHandleDrawable(left: Boolean): Drawable? {
        val attributes =
            intArrayOf(
                if (left) {
                    AndroidR.attr.textSelectHandleLeft
                } else {
                    AndroidR.attr.textSelectHandleRight
                },
            )
        val typedArray = surface.context.theme.obtainStyledAttributes(attributes)
        try {
            return typedArray.getDrawable(0)
        } finally {
            typedArray.recycle()
        }
    }

    fun showSelectionHandles(startRow: Int, startCol: Int, endRow: Int, endCol: Int, themeFgColor: Int) {
        if (startRow < 0 || startCol < 0 || endRow < 0 || endCol < 0) {
            hideSelectionHandles()
            return
        }
        val existingPopup = overlayPopup
        val existingContent = overlayContent
        val existingLiveness = overlayLiveness
        // 已有覆盖层存活：原地重定位。输入路径（ACTION_UP）与 Compose 副作用会在同一帧
        // 各调一次，事件流还在转发时也一样——绝不能拆窗重建（见 [PopupWindowLiveness]）。
        // 只有平台确实把窗口收走了（detach 且不再 pending）才拆掉重来。
        if (existingPopup != null &&
            existingContent != null &&
            existingLiveness != null &&
            existingPopup.isShowing &&
            existingLiveness.alive
        ) {
            // 复用同一窗口时必须撤销待拆请求：hide 在事件流转发中只登记延迟拆除，
            // 若新选区在流结束前又需要手柄，撤销后窗口才能继续服务。
            existingContent.consumeDeferredDismiss()
            val windowLocation = IntArray(2)
            surface.getLocationInWindow(windowLocation)
            val moved = overlayWindowX != windowLocation[0] || overlayWindowY != windowLocation[1]
            if (moved || existingPopup.width != surface.width || existingPopup.height != surface.height) {
                overlayWindowX = windowLocation[0]
                overlayWindowY = windowLocation[1]
                existingPopup.update(
                    windowLocation[0],
                    windowLocation[1],
                    surface.width,
                    surface.height,
                )
            }
            positionAllHandles(startRow, startCol, endRow, endCol)
            return
        }
        hideSelectionHandlesNow()
        // showAtLocation 需要窗口 token；在 Activity 结束过渡的帧中
        // 视图可能已 detach，调用会抛 BadTokenException。
        // `isAttachedToWindow` 在拆除回调内部仍为 true（见 [surface.viewDetached]），
        // 故两条件都要查——只凭它会把窗口建在正在销毁的 Activity 上。
        if (surface.viewDetached || !surface.isAttachedToWindow) return
        if (surface.width <= 0 || surface.height <= 0) return

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
            // 投递内拆窗会让本窗口的输入接收器先于 finishInputEvent 消失，
            // 平台随后的 finishInputEvent 无处可去，打出
            // `InputEventReceiver: ... already been disposed.`。推到下一轮 UI 循环
            // 再拆，事件投递与窗口拆除不再重叠。
            // 待拆请求在真正执行时才取：期间若新选区复用了同一窗口，它会撤销
            // 待拆请求，此时不得再拆。
            surface.post {
                // 新一轮触摸已经接管这扇窗口：等它结束时再拆，拆窗不得与事件投递重叠。
                if (content.streamForwarding) return@post
                val pending = content.consumeDeferredDismiss() ?: return@post
                dismissPopupQuietly(pending)
                if (overlayContent === content) clearOverlayState()
            }
        }
        val liveness = PopupWindowLiveness()
        overlayLiveness = liveness
        content.bindPopupWindowLiveness(liveness)

        val windowLocation = IntArray(2)
        surface.getLocationInWindow(windowLocation)
        overlayWindowX = windowLocation[0]
        overlayWindowY = windowLocation[1]
        val popup =
            PopupWindow(content, surface.width, surface.height).apply {
                isClippingEnabled = false
                setBackgroundDrawable(null)
                setAnimationStyle(0)
                // TYPE_APPLICATION_PANEL 而非 SUB_PANEL：覆盖层与选区菜单同在一个
                // 选区会话里重建/创建，层号必须把次序钉死——PANEL(1001) 低于
                // SUB_PANEL(1002)，菜单恒在覆盖层之上。两者同类型时按添加顺序叠加，
                // 覆盖层每次重建都晚于菜单，会把菜单压住：菜单被压住时落在条目上的
                // 点击穿透到终端、被当成「柄位外的轻击」清掉选区（现象：所有菜单
                // 按钮失效、点击菜单消失）。
                setWindowLayoutType(WindowManager.LayoutParams.TYPE_APPLICATION_PANEL)
                isSplitTouchEnabled = false
                // focusable=false 使键盘输入继续流向终端，
                // 同时覆盖层消费其边界内的触摸（由 dispatchTouchEvent 路由）。
                isFocusable = false
                isOutsideTouchable = false
            }
        liveness.markShown()
        try {
            popup.showAtLocation(surface, 0, windowLocation[0], windowLocation[1])
        } catch (exception: Exception) {
            // WindowManager.BadTokenException：Activity 在 isAttachedToWindow
            // 检查与 showAtLocation 之间被 detach。
            LogUtil.warn(TerminalSurface.TAG, "showSelectionHandles: overlay show failed", exception)
            liveness.markDetached()
            clearOverlayState()
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
        val viewportTopGrid = surface.currentViewportTopGrid()
        val visibleRow = (row - viewportTopGrid).coerceIn(0, surface.rows - 1)
        val anchorCol = if (which == HandleDrag.START) col else col + 1
        val (anchorXF, anchorYF) =
            gridToScreen(
                visibleRow + 1,
                anchorCol,
                viewportTopGrid = 0,
                cellWidth = surface.cellWidth,
                cellHeight = surface.cellHeight,
            )
        content.position(which, Math.round(anchorXF), Math.round(anchorYF))
        updateHitRect(which, Math.round(anchorXF), Math.round(anchorYF))
    }

    private fun positionAllHandles(startRow: Int, startCol: Int, endRow: Int, endCol: Int) {
        val viewportTopGrid = surface.currentViewportTopGrid()
        val visibleStartRow = (startRow - viewportTopGrid).coerceIn(0, surface.rows - 1)
        val (startX, startY) =
            gridToScreen(
                visibleStartRow + 1,
                startCol,
                viewportTopGrid = 0,
                cellWidth = surface.cellWidth,
                cellHeight = surface.cellHeight,
            )
        val visibleEndRow = (endRow - viewportTopGrid).coerceIn(0, surface.rows - 1)
        val (endX, endY) =
            gridToScreen(
                visibleEndRow + 1,
                endCol + 1,
                viewportTopGrid = 0,
                cellWidth = surface.cellWidth,
                cellHeight = surface.cellHeight,
            )
        val content = overlayContent ?: return
        content.position(HandleDrag.START, Math.round(startX), Math.round(startY))
        content.position(HandleDrag.END, Math.round(endX), Math.round(endY))
        updateHitRect(HandleDrag.START, Math.round(startX), Math.round(startY))
        updateHitRect(HandleDrag.END, Math.round(endX), Math.round(endY))
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
                (surface.width - handleW).coerceAtLeast(0),
            )
        val handleTop = anchorY.coerceIn(0, (surface.height - handleH).coerceAtLeast(0))
        val handleBounds = if (which == HandleDrag.START) startHandleRect else endHandleRect
        handleBounds.set(handleLeft, handleTop, handleLeft + handleW, handleTop + handleH)
        handleBounds.inset(-handleW / 4, -handleH / 4)
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
        clearOverlayState()
        dismissPopupQuietly(popup)
    }

    /** 清空覆盖层的视图/窗口引用与存活位（窗口本身的拆除由调用方负责）。 */
    private fun clearOverlayState() {
        overlayPopup = null
        overlayContent = null
        overlayLiveness = null
        startHandleRect.setEmpty()
        endHandleRect.setEmpty()
    }

    private fun dismissPopupQuietly(popup: PopupWindow?) {
        if (popup == null) return
        try {
            popup.dismiss()
            return
        } catch (exception: Exception) {
            LogUtil.warn(
                TerminalSurface.TAG,
                "dismissPopupQuietly: dismiss failed; forcing remove",
                exception,
            )
        }
        // 仅作兜底：dismiss 成功时 contentView 已被摘出 WindowManager，此时再
        // removeViewImmediate 必然抛 IllegalArgumentException。原先无条件执行它，
        // 于是**正常路径每次都抛一次异常**，而 catch 是空的——一次真实的窗口拆除
        // 失败与「本就无需拆除」在日志里完全无法区分（禁止空 catch / 不隐藏错误）。
        try {
            val windowManager =
                surface.context.getSystemService(Context.WINDOW_SERVICE)
                    as WindowManager
            windowManager.removeViewImmediate(popup.contentView)
        } catch (exception: Exception) {
            LogUtil.warn(TerminalSurface.TAG, "dismissPopupQuietly: forced remove failed", exception)
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
        private val leftDrawable: Drawable,
        private val rightDrawable: Drawable,
        val handleWidth: Int,
        val handleHeight: Int,
    ) : FrameLayout(surface.context) {
        private val startView = HandleView(leftDrawable)
        private val endView = HandleView(rightDrawable)

        /** 该布局当前拖拽所属的手柄；无拖拽时为 `null`。 */
        var dragOwner: HandleDrag? = null
        private var dragPointerLocked: Int? = null
        var streamForwarding: Boolean = false
            private set

        private var popupDeferredDismiss: PopupWindow? = null
        var onStreamEnded: (() -> Unit)? = null

        fun dismissOnStreamEnd(popup: PopupWindow?) {
            popupDeferredDismiss = popup
        }

        fun consumeDeferredDismiss(): PopupWindow? {
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
                    .coerceIn(0, (surface.width - handleWidth).coerceAtLeast(0))
            val targetY =
                anchorY.coerceIn(0, (surface.height - handleHeight).coerceAtLeast(0))
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
                surface.latchDragAnchor(which, dragPointerLocked)
            } else {
                dragOwner = null
                dragPointerLocked = null
                streamForwarding = true
                return surface.dispatchTouchEvent(event)
            }
            return true
        }

        private fun handleAt(pixelX: Int, pixelY: Int): HandleDrag? = when {
            !startHandleRect.isEmpty() && startHandleRect.contains(pixelX, pixelY) -> HandleDrag.START
            !endHandleRect.isEmpty() && endHandleRect.contains(pixelX, pixelY) -> HandleDrag.END
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
                    surface.driveHandleDragMove(event.getX(lockedIdx), event.getY(lockedIdx))
                }
                // 锁定的手指已消失：吞掉来自任何其他指针的游移（多指防漂移）。
                return true
            }
            if (streamForwarding) {
                return surface.dispatchTouchEvent(event)
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
                            surface.driveHandleDragMove(event.getX(lockedIdx), event.getY(lockedIdx))
                        }
                    }
                    surface.finishHandleDrag()
                    dragOwner = null
                    dragPointerLocked = null
                }
                return true
            }
            if (streamForwarding) {
                val handled = surface.dispatchTouchEvent(event)
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

        private inner class HandleView(private val drawable: Drawable) : View(surface.context) {
            override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
                setMeasuredDimension(handleWidth, handleHeight)
            }

            override fun onDraw(canvas: Canvas) {
                drawable.setBounds(0, 0, drawable.intrinsicWidth, drawable.intrinsicHeight)
                drawable.draw(canvas)
            }
        }
    }
}
