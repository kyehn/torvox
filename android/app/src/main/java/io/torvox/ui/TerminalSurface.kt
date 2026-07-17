// TODO(kotlin-2.4.0-false-positive): K2 smart-cast false positive, remove when upgrading Kotlin compiler
@file:Suppress("UNNECESSARY_SAFE_CALL")

package io.torvox.ui

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Rect
import android.graphics.SurfaceTexture
import android.graphics.drawable.Drawable
import android.util.AttributeSet
import android.util.Log
import android.view.ActionMode
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.PointerIcon
import android.view.ScaleGestureDetector
import android.view.Surface
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.widget.Magnifier
import android.widget.PopupWindow
import io.torvox.R
import io.torvox.SelectionMode
import io.torvox.TerminalViewModel
import io.torvox.runtime.InputBatchBuffer
import io.torvox.runtime.LogUtil

private val modifierBarHeightPx: Int by lazy {
    android.content.res.Resources.getSystem().displayMetrics.density.let { density ->
        (80f * density + 0.5f).toInt()
    }
}

internal fun isWordChar(c: Char): Boolean = c.isLetterOrDigit() || c == '_' || c == '-' || c == '.' || c == '/'

internal fun expandWordOnLine(
    line: String,
    col: Int,
): Pair<Int, Int> {
    if (col < 0) return Pair(0, 0)
    if (col >= line.length) return Pair(col, col)
    var pivot = col
    val ch = line[col]
    if (!isWordChar(ch)) {
        var left = col - 1
        while (left >= 0 && !isWordChar(line[left])) left--
        var right = col + 1
        while (right < line.length && !isWordChar(line[right])) right++
        pivot =
            when {
                left >= 0 && right < line.length -> {
                    if (col - left <= right - col) left else right
                }

                left >= 0 -> {
                    left
                }

                right < line.length -> {
                    right
                }

                else -> {
                    return Pair(col, col)
                }
            }
    }
    var startCol = pivot
    while (startCol > 0 && isWordChar(line[startCol - 1])) startCol--
    var endCol = pivot + 1
    while (endCol < line.length && isWordChar(line[endCol])) endCol++
    return Pair(startCol, endCol)
}

class TerminalSurface
    @JvmOverloads
    constructor(
        context: Context,
        attrs: AttributeSet? = null,
        defStyleAttr: Int = 0,
    ) : TextureView(context, attrs, defStyleAttr),
        TextureView.SurfaceTextureListener {
        companion object {
            private const val TAG = "TerminalSurface"
            private const val SWIPE_THRESHOLD_PIXELS = 500f
            private const val DEFAULT_ROWS = 24
            private const val DEFAULT_COLS = 80
            private const val DOUBLE_TAP_WINDOW_MS = 400L
            private const val ZOOM_THRESHOLD_LOW = 0.9f
            private const val ZOOM_THRESHOLD_HIGH = 1.1f
            private const val DRAWER_WIDTH_DP = 280

            private const val FLING_VELOCITY_DIVISOR = 100f
            private const val SUPPRESS_GRACE_PERIOD_NS = 50_000_000L
            private const val FLING_MAX_LINES = 50
            private const val SCROLL_END_DELAY_MS = 300L
            private const val FALLBACK_CELL_WIDTH = 8f
            private const val FALLBACK_CELL_HEIGHT = 16f
            private const val BACKSPACE_BYTE = 0x08.toByte()
            private const val DELETE_BYTE = 0x7F.toByte()
            private const val MENU_COPY = 1
            private const val MENU_PASTE = 2
            private const val MENU_SELECT_ALL = 3
            private const val EDGE_SCROLL_INTERVAL_MS = 50L
        }

        private fun getAccentColor(): Int = viewModel?.runtime?.accentColor ?: 0xFF2196F3.toInt()

        private var viewModel: TerminalViewModel? = null
        private var rows: Int = DEFAULT_ROWS
        private var cols: Int = DEFAULT_COLS
        private var surfaceWidthPixels: Int = 0
        private var surfaceHeightPixels: Int = 0
        private var isScrolling: Boolean = false
        private var scrollOffset: Int = 0
        private var lastImeBottom: Int = 0

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
            bridge.setSearchHighlights(data)
            viewModel?.runtime?.forceRender()
        }

        fun clearSearchHighlights() {
            val bridge = viewModel?.runtime?.bridge() ?: return
            bridge.clearSearchHighlights()
            // Force render after clearing highlights so the inverted colors disappear
            // immediately instead of lingering for a frame.
            viewModel?.runtime?.forceRender()
        }

        override fun onSurfaceTextureUpdated(surfaceTexture: SurfaceTexture) {
        }

        private var magnifier: Magnifier? = null
        private var lastConfiguredWidth = 0
        private var lastConfiguredHeight = 0

        var onScrollChanged: ((offset: Int) -> Unit)? = null
        var onScrollingStateChanged: ((isScrolling: Boolean) -> Unit)? = null
        var onSwipeLeft: (() -> Unit)? = null
        var onSwipeRight: (() -> Unit)? = null
        var onCopyRequested: ((text: String) -> Unit)? = null
        var onPasteRequested: (() -> Unit)? = null
        var onZoomChanged: ((increase: Boolean) -> Unit)? = null

        var drawerOpen: Boolean = false
            set(value) {
                field = value
                if (value) {
                    hideSelectionHandles()
                    hideContextMenu()
                }
            }

        /** When true, the search bar is shown and modifier bar is hidden — touches
         *  should reach the terminal surface instead of being excluded at the bottom. */
        var searchActive: Boolean = false

        private val drawerWidthPixels: Float by lazy { DRAWER_WIDTH_DP.toFloat() * resources.displayMetrics.density }

        private var startHandlePopup: PopupWindow? = null
        private var endHandlePopup: PopupWindow? = null
        private var cursorHandlePopup: PopupWindow? = null
        private var actionMode: ActionMode? = null
        private val startHandleRect = Rect()
        private val endHandleRect = Rect()

        @Suppress("CyclomaticComplexMethod", "ComplexCondition") // Acceptable — dispatches ~15 distinct gesture/intent types
        fun showSelectionHandles(
            startRow: Int,
            startCol: Int,
            endRow: Int,
            endCol: Int,
            themeFgColor: Int,
        ) {
            hideSelectionHandles()
            if (startRow < 0 || startCol < 0 || endRow < 0 || endCol < 0) return

            val loc = IntArray(2)
            getLocationInWindow(loc)

            val leftDrawable =
                androidx.core.content.ContextCompat
                    .getDrawable(context, R.drawable.text_select_handle_left_material)
                    ?: return
            leftDrawable.mutate()
            val rightDrawable =
                androidx.core.content.ContextCompat
                    .getDrawable(context, R.drawable.text_select_handle_right_material)
                    ?: return
            rightDrawable.mutate()
            leftDrawable.setTint(themeFgColor)
            rightDrawable.setTint(themeFgColor)
            val handleW = leftDrawable.intrinsicWidth
            selectionHandleWidth = handleW
            val handleH = leftDrawable.intrinsicHeight

            // START handle: positioned at bottom-right of start cell (Ghostty-Android pattern)
            val visibleStartRow = (startRow - scrollOffset).coerceIn(0, rows - 1)
            val startAnchorX = Math.round(startCol * cellWidth)
            val startAnchorY = Math.round((visibleStartRow + 1) * cellHeight)
            val startX = (startAnchorX - (handleW * 3) / 4).coerceIn(0, (width - handleW).coerceAtLeast(0))
            val startY = startAnchorY.coerceIn(0, (height - handleH).coerceAtLeast(0))
            val startView = createHandleViewWithDrawable(leftDrawable, HandleDrag.START)
            startHandlePopup =
                createHandlePopup(startView).apply {
                    showAtLocation(this@TerminalSurface, 0, loc[0] + startX, loc[1] + startY)
                }
            startHandleRect.set(startX, startY, startX + handleW, startY + handleH)
            startHandleRect.inset(-handleW / 4, -handleH / 4)

            // END handle: positioned at bottom-right of end cell
            val visibleEndRow = (endRow - scrollOffset).coerceIn(0, rows - 1)
            val endAnchorX = Math.round((endCol + 1) * cellWidth)
            val endAnchorY = Math.round((visibleEndRow + 1) * cellHeight)
            val endX = (endAnchorX - handleW / 4).coerceIn(0, (width - handleW).coerceAtLeast(0))
            val endY = endAnchorY.coerceIn(0, (height - handleH).coerceAtLeast(0))
            val endView = createHandleViewWithDrawable(rightDrawable, HandleDrag.END)
            endHandlePopup =
                createHandlePopup(endView).apply {
                    showAtLocation(this@TerminalSurface, 0, loc[0] + endX, loc[1] + endY)
                }
            endHandleRect.set(endX, endY, endX + handleW, endY + handleH)
            endHandleRect.inset(-handleW / 4, -handleH / 4)
        }

        private fun repositionHandle(
            which: HandleDrag,
            row: Int,
            col: Int,
        ) {
            val handleW = selectionHandleWidth
            if (handleW == 0) return
            val handleH = startHandlePopup?.contentView?.measuredHeight ?: return
            val loc = IntArray(2)
            getLocationInWindow(loc)
            val visibleRow = (row - scrollOffset).coerceIn(0, rows - 1)
            val anchorX =
                if (which == HandleDrag.START) {
                    Math.round(col * cellWidth)
                } else {
                    Math.round((col + 1) * cellWidth)
                }
            val anchorY = Math.round((visibleRow + 1) * cellHeight)
            val adjustedX =
                (anchorX - (if (which == HandleDrag.START) (handleW * 3) / 4 else handleW / 4))
                    .coerceIn(0, (width - handleW).coerceAtLeast(0))
            val clampedY = anchorY.coerceIn(0, (height - handleH).coerceAtLeast(0))
            val popupX = loc[0] + adjustedX
            val popupY = loc[1] + clampedY
            val popup = if (which == HandleDrag.START) startHandlePopup else endHandlePopup
            popup?.update(popupX, popupY, -1, -1)

            val rect = if (which == HandleDrag.START) startHandleRect else endHandleRect
            rect.set(
                adjustedX,
                clampedY,
                adjustedX + handleW,
                clampedY + handleH,
            )
            rect.inset(-handleW / 4, -handleH / 4)
        }

        fun hideSelectionToolbar() {
            actionMode?.hide(ActionMode.DEFAULT_HIDE_DURATION.toLong())
        }

        internal fun showContextMenu(
            row: Int,
            col: Int,
            hasSelection: Boolean,
            hasClipboard: Boolean,
            selectionStartRow: Int = row,
            selectionEndRow: Int = row,
            selectionStartCol: Int = col,
            selectionEndCol: Int = col,
        ) {
            hideContextMenu()
            val callback: ActionMode.Callback2 =
                object : ActionMode.Callback2() {
                    override fun onCreateActionMode(
                        mode: ActionMode,
                        menu: Menu,
                    ): Boolean {
                        if (hasSelection) {
                            menu.add(Menu.NONE, MENU_COPY, 0, android.R.string.copy)
                            menu.add(Menu.NONE, MENU_SELECT_ALL, 1, "Select All")
                        }
                        if (hasClipboard) {
                            menu.add(Menu.NONE, MENU_PASTE, 2, android.R.string.paste)
                        }
                        return true
                    }

                    override fun onPrepareActionMode(
                        mode: ActionMode,
                        menu: Menu,
                    ): Boolean {
                        styleActionMode(mode)
                        return false
                    }

                    override fun onActionItemClicked(
                        mode: ActionMode,
                        item: MenuItem,
                    ): Boolean {
                        when (item.itemId) {
                            MENU_COPY -> {
                                viewModel?.copySelectionToClipboard()
                                onCopyRequested?.invoke(getSelectedText())
                                viewModel?.clearSelection()
                                mode.finish()
                                return true
                            }

                            MENU_SELECT_ALL -> {
                                viewModel?.selectAll(scrollOffset)
                                mode.finish()
                                return true
                            }

                            MENU_PASTE -> {
                                onPasteRequested?.invoke()
                                mode.finish()
                                return true
                            }
                        }
                        return false
                    }

                    override fun onDestroyActionMode(mode: ActionMode) {
                        actionMode = null
                        viewModel?.let { viewModel ->
                            if (viewModel.state.value.selection.active) {
                                viewModel.clearSelection()
                            }
                        }
                        hideSelectionHandles()
                    }

                    override fun onGetContentRect(
                        mode: ActionMode,
                        view: View,
                        outRect: Rect,
                    ) {
                        try {
                            computeContentRect(outRect)
                        } catch (_: IllegalArgumentException) {
                            outRect.set(0, 0, 0, 0)
                        }
                    }
                }

            actionMode = startActionMode(callback, ActionMode.TYPE_FLOATING)
        }

        private fun computeContentRect(outRect: Rect) {
            if (width <= 0 || height <= 0) {
                outRect.set(0, 0, width.coerceAtLeast(0), height.coerceAtLeast(0))
                return
            }

            val selection = viewModel?.state?.value?.selection
            if (selection?.start == null || selection?.end == null) {
                outRect.set(0, 0, width, height)
                return
            }

            val rawLoRow = minOf(selection.start.row, selection.end.row) - scrollOffset
            val rawHiRow = maxOf(selection.start.row, selection.end.row) - scrollOffset

            val loRow: Int
            val hiRow: Int

            if (rawHiRow < 0 || rawLoRow >= rows) {
                loRow = rows / 2
                hiRow = rows / 2
            } else {
                loRow = rawLoRow.coerceIn(0, rows - 1)
                hiRow = rawHiRow.coerceIn(loRow, rows - 1)
            }

            val loCol = if (selection.start.row <= selection.end.row) selection.start.col else selection.end.col
            val hiCol = if (selection.start.row <= selection.end.row) selection.end.col else selection.start.col

            val imeInsetBottom =
                rootWindowInsets?.let {
                    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                        it
                            .getInsets(
                                android.view.WindowInsets.Type
                                    .ime(),
                            ).bottom
                    } else {
                        @Suppress("DEPRECATION")
                        val visibleFrame = Rect()
                        @Suppress("DEPRECATION")
                        getWindowVisibleDisplayFrame(visibleFrame)
                        height - visibleFrame.bottom
                    }
                } ?: 0
            val availableHeight = (height - imeInsetBottom).coerceAtLeast(1)
            val safeHeight = availableHeight
            if (safeHeight <= 0) {
                outRect.set(0, 0, width, height)
                return
            }
            val safeWidth = width.coerceAtLeast(1)

            val topOfSelection = Math.round(loRow * cellHeight)
            val bottomOfSelection = Math.round((hiRow + 1) * cellHeight)
            val left = Math.round(loCol * cellWidth)
            val right = Math.round((hiCol + 1) * cellWidth)

            // Position the anchor rect OUTSIDE the selection so the floating
            // ActionMode toolbar does not obscure the selected text.
            // If selection is in the lower half, anchor above it.
            // If in the upper half, anchor below it.
            val midPoint = safeHeight / 2
            if (topOfSelection > midPoint) {
                // Selection in lower half → anchor rect above the selection
                val anchorTop =
                    Math
                        .round(topOfSelection - cellHeight)
                        .coerceIn(0, safeHeight)
                val anchorBottom = topOfSelection.coerceIn(0, safeHeight)
                outRect.set(
                    left.coerceIn(0, safeWidth),
                    anchorTop,
                    right.coerceIn(0, safeWidth),
                    anchorBottom,
                )
            } else {
                // Selection in upper half → anchor rect below the selection
                val anchorTop = bottomOfSelection.coerceIn(0, safeHeight)
                val anchorBottom =
                    Math
                        .round(bottomOfSelection + cellHeight)
                        .coerceIn(0, safeHeight)
                outRect.set(
                    left.coerceIn(0, safeWidth),
                    anchorTop,
                    right.coerceIn(0, safeWidth),
                    anchorBottom,
                )
            }
        }

        fun hideContextMenu() {
            actionMode?.finish()
            actionMode = null
        }

        private fun styleActionMode(mode: ActionMode) {
            try {
                val accentColor = getAccentColor()
                for (i in 0 until mode.menu.size()) {
                    val item = mode.menu.getItem(i)
                    item.actionView?.let { view ->
                        (view as? android.widget.TextView)?.setTextColor(accentColor)
                    }
                }
            } catch (exception: Exception) {
                Log.w(TAG, "styleActionMode failed (non-critical)", exception)
            }
        }

        fun showCursorHandle(
            row: Int,
            col: Int,
            themeFgColor: Int,
        ) {
            hideCursorHandle()
            val loc = IntArray(2)
            getLocationInWindow(loc)

            val cursorDrawable =
                androidx.core.content.ContextCompat
                    .getDrawable(context, R.drawable.text_select_handle_left_material)
                    ?: return
            cursorDrawable.mutate()
            cursorDrawable.setTint(themeFgColor)
            val handleW = cursorDrawable.intrinsicWidth
            val handleH = cursorDrawable.intrinsicHeight

            val visibleRow = (row - scrollOffset).coerceIn(0, rows - 1)
            val cursorX = Math.round(col * cellWidth)
            val cursorY =
                Math
                    .round((visibleRow + 1) * cellHeight)
                    .coerceIn(0, (surfaceHeightPixels - handleH).coerceAtLeast(0))

            val cursorView = createHandleViewWithDrawable(cursorDrawable, HandleDrag.NONE)
            val popupX =
                (loc[0] + cursorX - handleW / 2)
                    .coerceIn(loc[0], (loc[0] + surfaceWidthPixels - handleW).coerceAtLeast(loc[0]))

            cursorHandlePopup =
                createHandlePopup(cursorView).apply {
                    showAtLocation(this@TerminalSurface, 0, popupX, loc[1] + cursorY)
                }
        }

        fun hideCursorHandle() {
            cursorHandlePopup?.dismiss()
            cursorHandlePopup = null
        }

        private fun createHandleViewWithDrawable(
            drawable: android.graphics.drawable.Drawable,
            which: HandleDrag,
        ): View =
            object : View(context) {
                override fun onMeasure(
                    widthMeasureSpec: Int,
                    heightMeasureSpec: Int,
                ) {
                    setMeasuredDimension(drawable.intrinsicWidth, drawable.intrinsicHeight)
                }

                override fun onDraw(canvas: android.graphics.Canvas) {
                    val drawableWidth = drawable.intrinsicWidth
                    val drawableHeight = drawable.intrinsicHeight
                    drawable.setBounds(0, 0, drawableWidth, drawableHeight)
                    drawable.draw(canvas)
                }

                @SuppressLint("ClickableViewAccessibility")
                override fun onTouchEvent(event: MotionEvent): Boolean {
                    if (which == HandleDrag.NONE) return super.onTouchEvent(event)
                    val surfaceLoc = IntArray(2)
                    this@TerminalSurface.getLocationOnScreen(surfaceLoc)
                    val localX = event.rawX - surfaceLoc[0]
                    val localY = event.rawY - surfaceLoc[1]
                    when (event.actionMasked) {
                        MotionEvent.ACTION_DOWN -> {
                            handleDragState = which
                            val col = (localX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                            val row = (localY / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                            if (which == HandleDrag.START) {
                                viewModel?.updateSelectionStart(row, col)
                            } else {
                                viewModel?.updateSelection(row, col)
                            }
                            return true
                        }

                        MotionEvent.ACTION_MOVE -> {
                            val col = (localX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                            val row = (localY / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                            if (which == HandleDrag.START) {
                                viewModel?.updateSelectionStart(row, col)
                            } else {
                                viewModel?.updateSelection(row, col)
                            }
                            repositionHandle(which, row, col)
                            return true
                        }

                        MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                            viewModel?.endSelection(scrollOffset)
                            handleDragState = HandleDrag.NONE
                            val selection = viewModel?.state?.value?.selection
                            if (selection?.start != null && selection?.end != null) {
                                hideSelectionHandles()
                                showSelectionHandles(
                                    selection.start.row,
                                    selection.start.col,
                                    selection.end.row,
                                    selection.end.col,
                                    getAccentColor(),
                                )
                            }
                            return true
                        }
                    }
                    return super.onTouchEvent(event)
                }
            }

        private fun createHandlePopup(contentView: View): PopupWindow {
            val popup = PopupWindow(context, null, android.R.attr.textSelectHandleWindowStyle)
            popup.setSplitTouchEnabled(true)
            popup.setClippingEnabled(false)
            popup.setWidth(android.view.ViewGroup.LayoutParams.WRAP_CONTENT)
            popup.setHeight(android.view.ViewGroup.LayoutParams.WRAP_CONTENT)
            popup.setBackgroundDrawable(null)
            popup.setAnimationStyle(0)
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
                popup.setWindowLayoutType(android.view.WindowManager.LayoutParams.TYPE_APPLICATION_SUB_PANEL)
                popup.setEnterTransition(null)
                popup.setExitTransition(null)
                popup.setTouchModal(false)
            }
            popup.setContentView(contentView)
            return popup
        }

        fun hideSelectionHandles() {
            startHandlePopup?.dismiss()
            startHandlePopup = null
            endHandlePopup?.dismiss()
            endHandlePopup = null
            hideSelectionToolbar()
            hideContextMenu()
            hideCursorHandle()
        }

        private var cachedCellWidth: Float = FALLBACK_CELL_WIDTH
        private var cachedCellHeight: Float = FALLBACK_CELL_HEIGHT

        val cellWidth: Float
            get() {
                val viewModelCellWidth = viewModel?.runtime?.cellWidth ?: 0f
                if (viewModelCellWidth > 0f) {
                    cachedCellWidth = viewModelCellWidth
                    return viewModelCellWidth
                }
                return cachedCellWidth
            }

        val cellHeight: Float
            get() {
                val viewModelCellHeight = viewModel?.runtime?.cellHeight ?: 0f
                if (viewModelCellHeight > 0f) {
                    cachedCellHeight = viewModelCellHeight
                    return viewModelCellHeight
                }
                return cachedCellHeight
            }

        @Volatile
        internal var isPaused = false

        @Volatile
        private var suppressUntilNanos = 0L

        private var pendingUnpauseRunnable: Runnable? = null

        @JvmField
        var isAfterLongPress = false

        var lastTapTime = 0L

        @JvmField
        var scaleFactor = 1.0f

        private enum class HandleDrag { NONE, START, END }

        private var handleDragState = HandleDrag.NONE
        private var selectionHandleWidth = 0
        private var longPressDragging = false
        private var longPressStartX = 0f
        private var longPressStartY = 0f

        private fun getClipboardManager(): ClipboardManager? {
            val clipboardManager = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            if (clipboardManager == null) {
                Log.w(TAG, "Clipboard service not available")
            }
            return clipboardManager
        }

        private fun safeHasPrimaryClip(clipboard: ClipboardManager? = getClipboardManager()): Boolean {
            if (clipboard == null) return false
            return try {
                clipboard.hasPrimaryClip()
            } catch (_: Exception) {
                false
            }
        }

        private val edgeScrollHandler = android.os.Handler(android.os.Looper.getMainLooper())
        private var edgeScrollRunning = false
        private var pendingEdgeScroll: Int = 0 // +1 = up, -1 = down, 0 = none
        private var edgeScrollRunnable: Runnable = Runnable { }

        val isSelectingText: Boolean
            get() =
                viewModel
                    ?.state
                    ?.value
                    ?.selection
                    ?.active == true

        private val gestureListener =
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(e: MotionEvent): Boolean = true

                override fun onShowPress(e: MotionEvent) {
                    isAfterLongPress = false
                }

                override fun onScroll(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    distanceX: Float,
                    distanceY: Float,
                ): Boolean {
                    if (isSelectingText) return false
                    val scrollbackLen = currentScrollbackLength()
                    if (!isScrolling) {
                        isScrolling = true
                        onScrollingStateChanged?.invoke(true)
                    }
                    // Treat one full cell-height of finger travel as one row of
                    // scroll, but scale so a full-viewport swipe scrolls the
                    // whole viewport. This keeps scrolling proportional and
                    // responsive instead of moving a single row per gesture.
                    val scale = (height.toFloat() / cellHeight.coerceAtLeast(1f)).coerceAtLeast(1f)
                    val rawAmount = (distanceY / cellHeight * scale / 4f).toInt()
                    val scrollAmount =
                        if (rawAmount > 0) {
                            maxOf(1, rawAmount)
                        } else if (rawAmount < 0) {
                            minOf(-1, rawAmount)
                        } else {
                            0
                        }
                    val newOffset = (scrollOffset + scrollAmount).coerceIn(0, scrollbackLen)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                    }
                    return true
                }

                override fun onFling(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    velocityX: Float,
                    velocityY: Float,
                ): Boolean {
                    if (isSelectingText) return false
                    val scrollbackLen = currentScrollbackLength()
                    val absX = kotlin.math.abs(velocityX)
                    val absY = kotlin.math.abs(velocityY)

                    if (absX > absY && absX > SWIPE_THRESHOLD_PIXELS) {
                        if (velocityX > 0) {
                            onSwipeRight?.invoke()
                        } else {
                            onSwipeLeft?.invoke()
                        }
                        return true
                    }

                    val flingAmount = (velocityY / FLING_VELOCITY_DIVISOR).toInt().coerceIn(-FLING_MAX_LINES, FLING_MAX_LINES)
                    val newOffset = (scrollOffset + flingAmount).coerceIn(0, scrollbackLen)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                    }
                    postDelayed({
                        isScrolling = false
                        onScrollingStateChanged?.invoke(false)
                    }, SCROLL_END_DELAY_MS)
                    return true
                }

                override fun onSingleTapUp(event: MotionEvent): Boolean {
                    if (isAfterLongPress) {
                        isAfterLongPress = false
                        longPressDragging = false
                        return true
                    }
                    if (isScrolling) {
                        // Just end the scroll state; do NOT reset scrollOffset to 0
                        // because that would undo the user's scroll on every tap,
                        // making scrollback feel unusable ("scrolling doesn't work").
                        isScrolling = false
                        onScrollingStateChanged?.invoke(false)
                        return true
                    }
                    if (isSelectingText) {
                        hideSelectionHandles()
                        viewModel?.clearSelection()
                        post {
                            @Suppress("DEPRECATION")
                            val controller =
                                androidx.core.view.ViewCompat
                                    .getWindowInsetsController(this@TerminalSurface)
                            controller?.hide(
                                androidx.core.view.WindowInsetsCompat.Type
                                    .ime(),
                            )
                        }
                        return true
                    }
                    hideContextMenu()
                    viewModel?.clearSelection()
                    viewModel?.resetCursorBlink()
                    suppressUntilNanos = 0L
                    keyboardRequested = true
                    requestFocus()
                    post {
                        @Suppress("DEPRECATION")
                        val controller =
                            androidx.core.view.ViewCompat
                                .getWindowInsetsController(this@TerminalSurface)
                        controller?.show(
                            androidx.core.view.WindowInsetsCompat.Type
                                .ime(),
                        )
                    }
                    return true
                }

                override fun onDoubleTap(event: MotionEvent): Boolean {
                    if (isSelectingText) {
                        viewModel?.clearSelection()
                        return true
                    }
                    val now = System.currentTimeMillis()
                    if (now - lastTapTime < DOUBLE_TAP_WINDOW_MS) {
                        val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                        val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                        viewModel?.setSelectionMode(SelectionMode.Line)
                        viewModel?.startSelection(row, 0)
                        val bridge = viewModel?.runtime?.bridge()
                        val scrollbackLength = bridge?.scrollbackLength()?.toInt() ?: 0
                        val line = bridge?.scrollbackLine((scrollbackLength - scrollOffset + row).toUInt()) ?: ""
                        viewModel?.updateSelection(row, line.length.coerceAtLeast(0))
                        viewModel?.endSelection(scrollOffset)
                        showSelectionHandles(row, 0, row, line.length.coerceAtLeast(0), getAccentColor())
                    } else {
                        startSelectionAt(event, expandToWord = true)
                        val currentSelection = viewModel?.state?.value?.selection
                        if (currentSelection?.active == true && currentSelection.start != null && currentSelection.end != null) {
                            showSelectionHandles(
                                currentSelection.start.row,
                                currentSelection.start.col,
                                currentSelection.end.row,
                                currentSelection.end.col,
                                getAccentColor(),
                            )
                        }
                    }
                    lastTapTime = now
                    return true
                }

                override fun onLongPress(event: MotionEvent) {
                    if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
                    isAfterLongPress = true
                    viewModel?.resetCursorBlink()
                    longPressDragging = true
                    longPressStartX = event.x
                    longPressStartY = event.y
                    val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                    val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                    val bridge = viewModel?.runtime?.bridge()
                    val scrollbackLength = bridge?.scrollbackLength()?.toInt() ?: 0
                    val line = bridge?.scrollbackLine((scrollbackLength - scrollOffset + row).toUInt()) ?: ""
                    handleLongPress(event.x, event.y)
                }
            }

        private val gestureDetector = GestureDetector(context, gestureListener)

        private val scaleDetector =
            ScaleGestureDetector(
                context,
                object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                    override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                        if (isSelectingText) return false
                        return true
                    }

                    override fun onScale(detector: ScaleGestureDetector): Boolean {
                        if (isSelectingText) return false
                        scaleFactor *= detector.scaleFactor
                        if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) {
                            val increase = scaleFactor > 1.0f
                            onZoomChanged?.invoke(increase)
                            scaleFactor = 1.0f
                        }
                        return true
                    }
                },
            )

        fun handleLongPress(
            x: Float,
            y: Float,
        ) {
            if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
            isAfterLongPress = true

            @Suppress("DEPRECATION")
            performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS)

            hideSelectionHandles()

            val col = (x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
            val row = (y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
            val bridge = viewModel?.runtime?.bridge()
            val scrollbackLength = bridge?.scrollbackLength()?.toInt() ?: 0
            val line = bridge?.scrollbackLine((scrollbackLength - scrollOffset + row).toUInt()) ?: ""
            val isOnEmptyArea = col >= line.length || line.isEmpty()
            val isOnWhitespace = !isOnEmptyArea && col < line.length && line[col].isWhitespace()

            if (isOnEmptyArea || isOnWhitespace) {
                val endCol = (col + 1).coerceAtMost(cols - 1)
                viewModel?.setSelectionMode(SelectionMode.Char)
                viewModel?.startSelection(row, col)
                viewModel?.updateSelection(row, endCol)
                viewModel?.endSelection(scrollOffset)
                showSelectionHandles(row, col, row, endCol, getAccentColor())

                Log.d(
                    "TorvoxSelection",
                    "LONG_PRESS empty/whitespace: row=$row col=$col endCol=$endCol " +
                        "isOnEmptyArea=$isOnEmptyArea isOnWhitespace=$isOnWhitespace " +
                        "mode=Char menu=PASTE_ONLY",
                )
            } else {
                val bounds =
                    viewModel?.runtime?.expandAndSetSelection(
                        row = (scrollbackLength - scrollOffset + row).toUInt(),
                        col = col.toUInt(),
                        mode = 4,
                    )
                val hasClipboard = safeHasPrimaryClip()

                val startRow: Int
                val startCol: Int
                val endRow: Int
                val endCol: Int

                if (bounds != null) {
                    val (start, end) = bounds
                    startRow = start.first.toInt()
                    startCol = start.second.toInt()
                    endRow = end.first.toInt()
                    endCol = end.second.toInt()
                    viewModel?.setSelectionMode(SelectionMode.Semantic)
                } else {
                    startRow = row
                    startCol = col
                    endRow = row
                    endCol = col
                }

                Log.d(
                    "TorvoxSelection",
                    "LONG_PRESS text: tapRow=$row tapCol=$col " +
                        "expanded start=($startRow,$startCol) end=($endRow,$endCol) " +
                        "mode=Word menu=FULL hasClipboard=$hasClipboard",
                )

                viewModel?.startSelection(startRow, startCol)
                viewModel?.updateSelection(endRow, endCol)
                viewModel?.endSelection(scrollOffset)
                showSelectionHandles(startRow, startCol, endRow, endCol, getAccentColor())
                // The selection menu is rendered by the Compose SelectionMenuOverlay,
                // driven by the view-model selection state. We intentionally do NOT
                // invoke the legacy ActionMode showContextMenu path here.
            }
        }

        private var cachedSurface: android.view.Surface? = null

        private var lastTouchX = 0f
        private var lastTouchY = 0f
        private var currentTouchX = 0f
        private var currentTouchY = 0f
        private var lastDragCol = 0
        private var lastDragRow = 0

        init {
            surfaceTextureListener = this
            isFocusable = true
            isFocusableInTouchMode = true
            setWillNotDraw(false)
            scaleDetector.isQuickScaleEnabled = false
            contentDescription = context.getString(R.string.terminal)
            importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES
            edgeScrollRunnable =
                Runnable {
                    if (!edgeScrollRunning) return@Runnable
                    when (pendingEdgeScroll) {
                        1 -> {
                            val scrollbackLen = currentScrollbackLength()
                            val newOffset = (scrollOffset + 1).coerceAtMost(scrollbackLen)
                            if (newOffset != scrollOffset) {
                                scrollOffset = newOffset
                                onScrollChanged?.invoke(scrollOffset)
                                val gridRow = scrollOffset
                                val curCol = (currentTouchX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                                if (handleDragState == HandleDrag.START) {
                                    viewModel?.updateSelectionStart(gridRow, curCol)
                                } else if (handleDragState == HandleDrag.END) {
                                    viewModel?.updateSelection(gridRow, curCol)
                                }
                            }
                        }

                        -1 -> {
                            val newOffset = (scrollOffset - 1).coerceAtLeast(0)
                            if (newOffset != scrollOffset) {
                                scrollOffset = newOffset
                                onScrollChanged?.invoke(scrollOffset)
                                val gridRow = scrollOffset + rows - 1
                                val curCol = (currentTouchX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                                if (handleDragState == HandleDrag.START) {
                                    viewModel?.updateSelectionStart(gridRow, curCol)
                                } else if (handleDragState == HandleDrag.END) {
                                    viewModel?.updateSelection(gridRow, curCol)
                                }
                            }
                        }
                    }
                    val selection = viewModel?.state?.value?.selection
                    if (selection?.start != null && selection?.end != null) {
                        repositionHandle(HandleDrag.START, selection.start.row, selection.start.col)
                        repositionHandle(HandleDrag.END, selection.end.row, selection.end.col)
                    }
                    if (edgeScrollRunning) {
                        edgeScrollHandler.postDelayed(edgeScrollRunnable, EDGE_SCROLL_INTERVAL_MS)
                    }
                }
        }

        private var keyboardRequested = false

        fun finishComposing() {
            currentInputConnection?.let { ic ->
                ic.finishComposingText()
            }
            coalescer.clearComposing()
        }

        fun restoreKeyboardFocus() {
            keyboardRequested = true
            requestFocus()
            post {
                @Suppress("DEPRECATION")
                val controller =
                    androidx.core.view.ViewCompat
                        .getWindowInsetsController(this)
                controller?.show(
                    androidx.core.view.WindowInsetsCompat.Type
                        .ime(),
                )
            }
        }

        override fun onCheckIsTextEditor(): Boolean = keyboardRequested

        override fun onResolvePointerIcon(
            event: MotionEvent,
            pointerIndex: Int,
        ): PointerIcon = PointerIcon.getSystemIcon(context, PointerIcon.TYPE_TEXT)

        override fun onWindowFocusChanged(hasFocus: Boolean) {
            super.onWindowFocusChanged(hasFocus)
            viewModel?.runtime?.focusChange(hasFocus)
            if (!hasFocus) {
                isPaused = true
                coalescer.reset()
                currentInputConnection?.let { ic ->
                    ic.finishComposingText()
                }
                suppressUntilNanos = System.nanoTime() + SUPPRESS_GRACE_PERIOD_NS
            }
            if (hasFocus) {
                // Reset pause state but do NOT call finishComposingText — the IME may be
                // mid-composition; aborting it here causes the IME to lose sync and
                // produce duplicate text, extra spaces, or silently drop input.
                isPaused = false
                suppressUntilNanos = System.nanoTime() + SUPPRESS_GRACE_PERIOD_NS
            }
        }

        override fun onApplyWindowInsets(insets: WindowInsets): WindowInsets {
            val result = super.onApplyWindowInsets(insets)
            val imeBottom = insets.getInsets(WindowInsets.Type.ime()).bottom
            if (imeBottom != lastImeBottom) {
                lastImeBottom = imeBottom
                val cellWidth = viewModel?.runtime?.cellWidth ?: return result
                val cellHeight = viewModel?.runtime?.cellHeight ?: return result
                if (cellWidth <= 0f || cellHeight <= 0f) return result
                if (imeBottom > 0) {
                    // ModifierBar is now a Compose overlay; terminal grid must not render rows behind it.
                    val availableHeight = height - imeBottom - modifierBarHeightPx
                    if (availableHeight <= 0) return result
                    val newCols = (width.toFloat() / cellWidth).toInt().coerceAtLeast(1)
                    val newRows = (availableHeight.toFloat() / cellHeight).toInt().coerceAtLeast(1)
                    if (newRows != this.rows || newCols != this.cols) {
                        viewModel?.runtime?.resize(newRows, newCols)
                        this.rows = newRows
                        this.cols = newCols
                    }
                } else if (imeBottom == 0) {
                    val fullRows = ((height - modifierBarHeightPx).toFloat() / cellHeight).toInt().coerceAtLeast(1)
                    val fullCols = (width.toFloat() / cellWidth).toInt().coerceAtLeast(1)
                    if (fullRows != this.rows || fullCols != this.cols) {
                        viewModel?.runtime?.resize(fullRows, fullCols)
                        this.rows = fullRows
                        this.cols = fullCols
                    }
                }
            }
            return result
        }

        fun initialize(viewModel: TerminalViewModel) {
            this.viewModel = viewModel
        }

        fun postDelayedUnpause(delayMillis: Long) {
            pendingUnpauseRunnable?.let { removeCallbacks(it) }
            pendingUnpauseRunnable =
                Runnable {
                    pendingUnpauseRunnable = null
                    if (hasWindowFocus()) {
                        isPaused = false
                    }
                }.also { postDelayed(it, delayMillis) }
        }

        fun setDimensions(
            rows: Int,
            cols: Int,
        ) {
            this.rows = rows
            this.cols = cols
        }

        private fun currentScrollbackLength(): Int {
            val viewModel = viewModel ?: return 0
            val bridge = viewModel.runtime.bridge() ?: return 0
            return try {
                bridge.scrollbackLength().toInt()
            } catch (error: Exception) {
                LogUtil.e(TAG, "scrollbackLength query failed", error)
                0
            }
        }

        fun setScrollOffset(offset: Int) {
            this.scrollOffset = offset.coerceIn(0, currentScrollbackLength())
            onScrollChanged?.invoke(this.scrollOffset)
        }

        fun scrollToRow(row: Int) {
            val scrollbackLen = currentScrollbackLength()
            val targetOffset = (scrollbackLen - row).coerceIn(0, scrollbackLen)
            if (targetOffset != scrollOffset) {
                scrollOffset = targetOffset
                onScrollChanged?.invoke(scrollOffset)
                // Signal the render thread (vsync-paced) instead of blocking the UI
                // thread with a synchronous GPU render on every scroll event.
                viewModel?.runtime?.forceRender()
            }
        }

        fun getScrollOffset(): Int = scrollOffset

        fun getMaxScrollOffset(): Int = currentScrollbackLength()

        fun getRows(): Int = rows

        fun getCols(): Int = cols

        fun isCurrentlyScrolling(): Boolean = isScrolling

        private fun recomputeRowsColsImmediate(
            width: Int,
            height: Int,
        ) {
            val viewModel = viewModel
            if (viewModel != null) {
                val cellWidth = viewModel.runtime.cellWidth
                val cellHeight = viewModel.runtime.cellHeight
                if (cellWidth > 0f && cellHeight > 0f) {
                    cols = (width.toFloat() / cellWidth).toInt().coerceAtLeast(1)
                    rows = (height.toFloat() / cellHeight).toInt().coerceAtLeast(1)
                    return
                }
            }
            if (lastConfiguredWidth > 0 && lastConfiguredHeight > 0 && rows > 0 && cols > 0) {
                val cellWidthPx = lastConfiguredWidth.toFloat() / cols
                val cellHeightPx = lastConfiguredHeight.toFloat() / rows
                cols = (width.toFloat() / cellWidthPx).toInt().coerceAtLeast(1)
                rows = (height.toFloat() / cellHeightPx).toInt().coerceAtLeast(1)
            }
        }

        fun consumePendingInput(): ByteArray? = viewModel?.consumePendingInput()

        private val inputBatchBuffer = InputBatchBuffer({ data -> viewModel?.writeToPty(data) })
        private val coalescer = InputCoalescer({ data -> inputBatchBuffer.write(data) })
        private var currentInputConnection: InputConnection? = null

        override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
            val mode = viewModel?.state?.value?.keyboardMode ?: KeyboardMode.Raw
            mode.toEditorInfo(outAttrs)
            val connection =
                object : BaseInputConnection(this, true) {
                    // Tracks in-progress IME composition so deltas reconcile instead
                    // of being dropped. Modeled on Haven's WaylandDesktopView
                    // setComposingText / commitText (core/wayland/.../WaylandDesktopView.kt:296-329).
                    private var composingBuffer: String = ""

                    private fun encodeAndSend(
                        text: String,
                        ctrlActive: Boolean,
                        altActive: Boolean,
                    ) {
                        coalescer.send(
                            TerminalInputEncoder.encodeCommittedText(
                                text = text,
                                ctrlActive = ctrlActive,
                                altActive = altActive,
                                bracketedPaste = false,
                            ),
                        )
                    }

                    override fun setComposingText(
                        text: CharSequence?,
                        newCursorPosition: Int,
                    ): Boolean {
                        if (isPaused || System.nanoTime() < suppressUntilNanos) {
                            composingBuffer = ""
                            coalescer.clearComposing()
                            return true
                        }
                        val newComposing = text?.toString() ?: ""
                        when {
                            newComposing == composingBuffer -> {
                                // No change — nothing to reconcile.
                            }

                            newComposing.startsWith(composingBuffer) -> {
                                // Composition grew: send only the appended characters.
                                encodeAndSend(
                                    newComposing.substring(composingBuffer.length),
                                    ctrlActive = false,
                                    altActive = false,
                                )
                            }

                            composingBuffer.startsWith(newComposing) -> {
                                // Composition contracted: backspace the removed characters.
                                viewModel?.writeToPty(
                                    ByteArray(composingBuffer.length - newComposing.length) { BACKSPACE_BYTE },
                                )
                            }

                            else -> {
                                // Diverged: replace the whole composing run.
                                viewModel?.writeToPty(ByteArray(composingBuffer.length) { BACKSPACE_BYTE })
                                encodeAndSend(newComposing, ctrlActive = false, altActive = false)
                            }
                        }
                        composingBuffer = newComposing
                        coalescer.updateComposingText(newComposing)
                        return true
                    }

                    override fun finishComposingText(): Boolean {
                        composingBuffer = ""
                        coalescer.clearComposing()
                        return true
                    }

                    override fun commitText(
                        text: CharSequence?,
                        newCursorPosition: Int,
                    ): Boolean {
                        if (isPaused || System.nanoTime() < suppressUntilNanos) {
                            composingBuffer = ""
                            coalescer.clearComposing()
                            return true
                        }
                        val committedText = text?.toString() ?: return false
                        val terminalViewModel = viewModel
                        val state = terminalViewModel?.state?.value
                        val ctrlActive =
                            state?.ctrlState == ModifierState.Locked || state?.ctrlState == ModifierState.Once
                        val altActive =
                            state?.altState == ModifierState.Locked || state?.altState == ModifierState.Once

                        if (composingBuffer.isNotEmpty()) {
                            if (committedText == composingBuffer) {
                                // Already forwarded via composing deltas; do not resend.
                            } else {
                                terminalViewModel?.writeToPty(
                                    ByteArray(composingBuffer.length) { BACKSPACE_BYTE },
                                )
                                encodeAndSend(committedText, ctrlActive, altActive)
                            }
                            composingBuffer = ""
                        } else {
                            encodeAndSend(committedText, ctrlActive, altActive)
                        }
                        coalescer.clearComposing()
                        terminalViewModel?.consumeOneShotModifiers()
                        terminalViewModel?.resetCursorBlink()
                        return true
                    }

                    override fun sendKeyEvent(event: KeyEvent): Boolean {
                        if (isPaused || System.nanoTime() < suppressUntilNanos) {
                            return true
                        }
                        return if (event.action == KeyEvent.ACTION_DOWN) {
                            viewModel?.resetCursorBlink()
                            handleKeyEvent(event)
                        } else {
                            true
                        }
                    }

                    override fun deleteSurroundingText(
                        beforeLength: Int,
                        afterLength: Int,
                    ): Boolean {
                        if (isPaused || System.nanoTime() < suppressUntilNanos) {
                            return true
                        }
                        if (beforeLength > 0) {
                            val bs = ByteArray(beforeLength) { BACKSPACE_BYTE }
                            viewModel?.writeToPty(bs)
                        }
                        if (afterLength > 0) {
                            val del = ByteArray(afterLength) { DELETE_BYTE }
                            viewModel?.writeToPty(del)
                        }
                        return true
                    }
                }
            currentInputConnection = connection
            return connection
        }

        fun pasteFromClipboardDirect() {
            val clipboard = getClipboardManager() ?: return
            if (!clipboard.hasPrimaryClip()) return
            val clipboardText = clipboard.primaryClip?.getItemAt(0)?.text ?: return
            val data = clipboardText.toString().replace("\n", "\r").toByteArray()
            viewModel?.writeToPty(data)
        }

        fun getSelectedText(): String {
            val selection = viewModel?.state?.value?.selection ?: return ""
            if (!selection.active || selection.start == null || selection.end == null) return ""
            val bridge = viewModel?.runtime?.bridge() ?: return ""
            val scrollbackLength = bridge.scrollbackLength().toInt()
            val loRow = minOf(selection.start.row, selection.end.row)
            val hiRow = maxOf(selection.start.row, selection.end.row)
            val loCol = if (selection.start.row <= selection.end.row) selection.start.col else selection.end.col
            val hiCol = if (selection.start.row <= selection.end.row) selection.end.col else selection.start.col
            val builder = StringBuilder()
            for (row in loRow..hiRow) {
                val line = bridge.scrollbackLine((scrollbackLength - scrollOffset + row).toUInt()) ?: continue
                val startCol = if (row == loRow) loCol else 0
                val endCol = if (row == hiRow) hiCol.coerceAtMost(line.length) else line.length
                if (startCol < endCol) {
                    builder.appendLine(line.substring(startCol, endCol))
                }
            }
            return builder.toString().trimEnd('\n')
        }

        private fun startSelectionAt(
            event: MotionEvent,
            expandToWord: Boolean = false,
        ) {
            val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
            val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))

            if (expandToWord) {
                // Use the core-backed expansion (CJK / URL aware) so double-tap word
                // selection matches long-press exactly — no divergent client logic.
                val bridge = viewModel?.runtime?.bridge()
                val scrollbackLength = bridge?.scrollbackLength()?.toInt() ?: 0
                val bounds =
                    viewModel?.runtime?.expandAndSetSelection(
                        row = (scrollbackLength - scrollOffset + row).toUInt(),
                        col = col.toUInt(),
                        mode = 1,
                    )
                if (bounds != null) {
                    val (start, end) = bounds
                    viewModel?.setSelectionMode(SelectionMode.Word)
                    viewModel?.startSelection(start.first.toInt(), start.second.toInt())
                    viewModel?.updateSelection(end.first.toInt(), end.second.toInt())
                    viewModel?.endSelection(scrollOffset)
                    Log.d(
                        "TorvoxSelection",
                        "DOUBLE_TAP word: tapRow=$row tapCol=$col " +
                            "expanded start=(${start.first},${start.second}) end=(${end.first},${end.second})",
                    )
                } else {
                    viewModel?.startSelection(row, col)
                }
            } else {
                viewModel?.startSelection(row, col)
            }

            try {
                magnifier = magnifier ?: Magnifier.Builder(this@TerminalSurface).build()
                magnifier?.show(event.rawX, event.rawY)
            } catch (exception: Exception) {
                Log.w(TAG, "magnifier show failed (non-critical)", exception)
            }
        }

        private fun modifierBitmask(event: KeyEvent): Byte {
            val state = viewModel?.state?.value
            var mask = 0
            if (event.isShiftPressed) mask = mask or 1
            if (event.isAltPressed || state?.altState == ModifierState.Locked || state?.altState == ModifierState.Once) {
                mask = mask or 2
            }
            if (event.isCtrlPressed || state?.ctrlState == ModifierState.Locked || state?.ctrlState == ModifierState.Once) {
                mask = mask or 4
            }
            if (event.isMetaPressed) mask = mask or 8
            return mask.toByte()
        }

        override fun onKeyDown(
            keyCode: Int,
            event: KeyEvent,
        ): Boolean {
            val terminalViewModel = viewModel
            val bridge = terminalViewModel?.runtime?.bridge()
            if (bridge != null) {
                val modifiers = modifierBitmask(event)
                val action: Byte = 0 // KeyEvent.ACTION_DOWN = 0
                val unicodeChar = event.unicodeChar
                val unshiftedChar = event.getUnicodeChar(event.metaState and KeyEvent.META_SHIFT_MASK.inv())
                val success = bridge.processKeyEvent(keyCode, modifiers, action, unicodeChar, unshiftedChar)
                if (success) {
                    terminalViewModel.consumeOneShotModifiers()
                    return true
                }
            }
            return super.onKeyDown(keyCode, event)
        }

        override fun onKeyUp(
            keyCode: Int,
            event: KeyEvent,
        ): Boolean {
            val terminalViewModel = viewModel
            val bridge = terminalViewModel?.runtime?.bridge()
            if (bridge != null) {
                val modifiers = modifierBitmask(event)
                val action: Byte = 1 // KeyEvent.ACTION_UP = 1
                val unicodeChar = event.unicodeChar
                val unshiftedChar = event.getUnicodeChar(event.metaState and KeyEvent.META_SHIFT_MASK.inv())
                bridge.processKeyEvent(keyCode, modifiers, action, unicodeChar, unshiftedChar)
            }
            return true
        }

        private fun handleKeyEvent(event: KeyEvent): Boolean = onKeyDown(event.keyCode, event)

        @Suppress("CyclomaticComplexMethod", "LongMethod", "NestedBlockDepth") // Acceptable — dispatches ~15 distinct gesture/intent types
        override fun onTouchEvent(event: MotionEvent): Boolean {
            if (!touchEnabled) {
                scaleDetector.onTouchEvent(event)
                gestureDetector.onTouchEvent(event)
                return false
            }
            if (event.action == MotionEvent.ACTION_DOWN) {
                lastTouchX = event.x
                lastTouchY = event.y
                parent?.requestDisallowInterceptTouchEvent(true)
            }
            if (drawerOpen && event.x < drawerWidthPixels) {
                return false
            }

            // No longer needed — ModifierBar is now a Compose overlay at higher
            // z-index which naturally intercepts touches in the mod bar zone.

            val fromMouse = event.isFromSource(InputDevice.SOURCE_MOUSE)

            if (fromMouse) {
                when {
                    event.isButtonPressed(MotionEvent.BUTTON_SECONDARY) -> {
                        if (event.action == MotionEvent.ACTION_DOWN) {
                            viewModel?.clearSelection()
                            startSelectionAt(event, expandToWord = true)
                        }
                        return true
                    }

                    event.isButtonPressed(MotionEvent.BUTTON_TERTIARY) -> {
                        if (event.action == MotionEvent.ACTION_DOWN) {
                            pasteFromClipboardDirect()
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
                        val touchX = event.x
                        val touchY = event.y
                        val touchCol = (touchX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                        val touchRow = (touchY / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                        val gridRow = touchRow + scrollOffset
                        if (!startHandleRect.isEmpty() && startHandleRect.contains(touchX.toInt(), touchY.toInt())) {
                            handleDragState = HandleDrag.START
                            viewModel?.updateSelectionStart(gridRow, touchCol)
                            return true
                        } else if (!endHandleRect.isEmpty() && endHandleRect.contains(touchX.toInt(), touchY.toInt())) {
                            handleDragState = HandleDrag.END
                            viewModel?.updateSelection(gridRow, touchCol)
                            return true
                        } else {
                            viewModel?.clearSelection()
                            hideSelectionHandles()
                            hideContextMenu()
                        }
                    }
                }

                MotionEvent.ACTION_MOVE -> {
                    if (isSelectingText && handleDragState != HandleDrag.NONE) {
                        val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                        val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                        lastDragCol = col
                        lastDragRow = row
                        currentTouchX = event.x
                        currentTouchY = event.y

                        hideSelectionToolbar()

                        if (event.y < cellHeight / 2) {
                            if (!edgeScrollRunning) {
                                edgeScrollRunning = true
                                pendingEdgeScroll = 1
                                edgeScrollHandler.postDelayed(edgeScrollRunnable, EDGE_SCROLL_INTERVAL_MS)
                            }
                        } else if (event.y >= surfaceHeightPixels - cellHeight / 2) {
                            if (!edgeScrollRunning) {
                                edgeScrollRunning = true
                                pendingEdgeScroll = -1
                                edgeScrollHandler.postDelayed(edgeScrollRunnable, EDGE_SCROLL_INTERVAL_MS)
                            }
                        } else {
                            edgeScrollRunning = false
                            pendingEdgeScroll = 0
                            edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
                            val gridRow = row + scrollOffset
                            if (handleDragState == HandleDrag.START) {
                                viewModel?.updateSelectionStart(gridRow, col)
                            } else {
                                viewModel?.updateSelection(gridRow, col)
                            }
                            repositionHandle(handleDragState, gridRow, col)
                        }
                    } else if (longPressDragging && isSelectingText) {
                        val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                        val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                        val gridRow = row + scrollOffset
                        viewModel?.updateSelection(gridRow, col)
                        val sel = viewModel?.state?.value?.selection
                        if (sel?.start != null && sel?.end != null) {
                            showSelectionHandles(sel.start.row, sel.start.col, sel.end.row, sel.end.col, getAccentColor())
                        }
                    }
                }

                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    if (longPressDragging) {
                        longPressDragging = false
                        val sel = viewModel?.state?.value?.selection
                        if (sel?.start != null && sel?.end != null) {
                            viewModel?.endSelection(scrollOffset)
                            val clipboard = getClipboardManager()
                            showSelectionHandles(sel.start.row, sel.start.col, sel.end.row, sel.end.col, getAccentColor())
                        }
                    }
                    edgeScrollRunning = false
                    pendingEdgeScroll = 0
                    edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
                    if (isSelectingText && handleDragState != HandleDrag.NONE) {
                        viewModel?.endSelection(scrollOffset)
                        val selection = viewModel?.state?.value?.selection
                        if (selection?.start != null && selection?.end != null) {
                            val clipboard = getClipboardManager()
                            hideSelectionHandles()
                            showSelectionHandles(
                                selection.start.row,
                                selection.start.col,
                                selection.end.row,
                                selection.end.col,
                                getAccentColor(),
                            )
                            // The selection menu is rendered by the Compose
                            // SelectionMenuOverlay, driven by the view-model state.
                        }
                        // Flush the new selection state to the Rust renderer so it
                        // paints the selection highlight at the correct position.
                        viewModel?.runtime?.forceRender()
                    }
                    handleDragState = HandleDrag.NONE
                    try {
                        magnifier?.dismiss()
                    } catch (exception: Exception) {
                        Log.w(TAG, "magnifier dismiss failed (non-critical)", exception)
                    }
                    magnifier = null
                    scaleFactor = 1.0f
                }
            }
            return true
        }

        // ── SurfaceTextureListener ─────────────────────────────────────────

        @Suppress("CyclomaticComplexMethod") // Acceptable — dispatches ~15 distinct gesture/intent types
        override fun onSizeChanged(
            width: Int,
            height: Int,
            previousWidth: Int,
            previousHeight: Int,
        ) {
            super.onSizeChanged(width, height, previousWidth, previousHeight)
            if (width <= 0 || height <= 0) return
            if (width == previousWidth && height == previousHeight && previousWidth != 0) return

            val surfaceTextureLocal = surfaceTexture ?: return
            surfaceTextureLocal.setDefaultBufferSize(width, height)

            surfaceWidthPixels = width
            surfaceHeightPixels = height
            recomputeRowsColsImmediate(width, height)
            // Resize the GPU swapchain synchronously and immediately so the rendered
            // frame always matches the new view size. The TextureView scales its
            // SurfaceTexture to the view bounds; if the wgpu/swapchain buffer stayed
            // at the old size for even a few frames (e.g. while the IME animates),
            // the stale buffer would be non-uniformly scaled -> the text would
            // visibly stretch/compress. Immediate resize keeps buffer == view at all
            // times, eliminating the artifact.
            applySurfaceResize(width, height)
        }

        /**
         * Reconfigure the native (wgpu) surface + grid to [width]x[height] right now.
         * Idempotent: a no-op when the size already matches the last configured size.
         * Must run on the main thread (holds the bridge surface lock while the render
         * thread may briefly contend, but never deadlocks).
         */
        private fun applySurfaceResize(
            width: Int,
            height: Int,
        ) {
            if (width <= 0 || height <= 0) return
            if (width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0) return
            val terminalViewModel = viewModel ?: return
            terminalViewModel.surfaceWidth = width
            terminalViewModel.surfaceHeight = height

            terminalViewModel.runtime.bridge()?.setSurfaceSize(width, height)
            applyResizeNormal(width, height, terminalViewModel)
        }

        private fun applyResizeNormal(
            width: Int,
            height: Int,
            terminalViewModel: TerminalViewModel,
        ) {
            terminalViewModel.runtime.recomputeGrid(width, height)
            val surface =
                cachedSurface ?: (
                    surfaceTexture?.let {
                        Surface(it).also {
                            cachedSurface = it
                            terminalViewModel.currentSurface = it
                        }
                    }
                )
            if (surface == null) {
                Log.w(TAG, "applySurfaceResize: no cachedSurface yet, deferring")
                return
            }
            terminalViewModel.currentSurface = surface
            val windowPointer = terminalViewModel.runtime.getNativeWindowPtr(surface)
            if (windowPointer == 0L) {
                Log.w(TAG, "applySurfaceResize: windowPointer=0, skipping")
                return
            }
            terminalViewModel.runtime.updateNativeWindow(windowPointer, width, height)
            val runtimeState = terminalViewModel.runtime.state.value
            if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                rows = runtimeState.rows
                cols = runtimeState.cols
            }
            lastConfiguredWidth = width
            lastConfiguredHeight = height
        }

        override fun onSurfaceTextureAvailable(
            surfaceTexture: SurfaceTexture,
            width: Int,
            height: Int,
        ) {
            cachedSurface?.release()
            val textureSurface = Surface(surfaceTexture).also { cachedSurface = it }
            surfaceWidthPixels = width
            surfaceHeightPixels = height
            viewModel?.let { terminalViewModel ->
                terminalViewModel.surfaceWidth = width
                terminalViewModel.surfaceHeight = height
                terminalViewModel.currentSurface = textureSurface
                val isRunning = terminalViewModel.runtime.state.value.isRunning
                if (!isRunning) {
                    terminalViewModel.startRuntime(textureSurface, width, height)
                } else {
                    val windowPointer = terminalViewModel.runtime.getNativeWindowPtr(textureSurface)
                    if (windowPointer != 0L) {
                        terminalViewModel.runtime.updateNativeWindow(windowPointer, width, height)
                        terminalViewModel.runtime.recomputeGrid(width, height)
                        terminalViewModel.runtime.resumeRendering()
                        val runtimeState = terminalViewModel.runtime.state.value
                        if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                            rows = runtimeState.rows
                            cols = runtimeState.cols
                        }
                    }
                }
                lastConfiguredWidth = width
                lastConfiguredHeight = height
            }
        }

        override fun onSurfaceTextureSizeChanged(
            surfaceTexture: SurfaceTexture,
            width: Int,
            height: Int,
        ) {
            surfaceWidthPixels = width
            surfaceHeightPixels = height
            recomputeRowsColsImmediate(width, height)
            // Always resize the swapchain when the surface size changes,
            // including IME show/hide (height-only changes). The old approach
            // of skipping height-only changes caused text stretch/compression
            // because the GPU rendered into a stale-sized buffer while the
            // TextureView had already resized, producing non-uniform scaling.
            applySurfaceResize(width, height)
        }

        override fun onSurfaceTextureDestroyed(surfaceTexture: SurfaceTexture): Boolean {
            viewModel?.runtime?.onSurfaceDestroyed()
            viewModel?.runtime?.releaseAllGpuSurfaces()
            if (isAttachedToWindow) {
                hideContextMenu()
            }
            cachedSurface?.release()
            cachedSurface = null
            lastConfiguredWidth = 0
            lastConfiguredHeight = 0
            viewModel?.currentSurface = null
            return true
        }
    }
