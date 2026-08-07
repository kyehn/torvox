// TODO(kotlin-2.4.0-false-positive): K2 smart-cast false positive, remove when upgrading Kotlin compiler
@file:Suppress("UNNECESSARY_SAFE_CALL")

package terminal.emulator.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Rect
import android.graphics.drawable.Drawable
import android.util.AttributeSet
import android.util.Log
import android.view.ActionMode
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PointerIcon
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.widget.Magnifier
import android.widget.LinearLayout
import android.widget.PopupWindow
import kotlin.math.roundToInt
import android.widget.TextView
import terminal.emulator.R
import terminal.emulator.SelectionMode
import terminal.emulator.TerminalViewModel
import terminal.emulator.TouchClass
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.KeyboardMode
import terminal.emulator.input.ModifierState
import terminal.emulator.input.toEditorInfo
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.ClipboardPaster
import terminal.emulator.runtime.InputBatchBuffer
import terminal.emulator.runtime.LogUtil

// Approximate height reserved for the ModifierBar overlay when computing
// the terminal grid (see applyGridResize). The bar itself is ~36dp of
// buttons + padding + navigationBarsPadding; 80dp is a deliberately safe
// over-estimate. Inert while Bridge.getCellWidth is an ADR-0007 stub;
// recalibrate against the real ModifierBar layout when rendering lands
// (round-113).
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

@Suppress("TooManyFunctions")
class TerminalSurface
@JvmOverloads
constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0,
) : SurfaceView(context, attrs, defStyleAttr),
    SurfaceHolder.Callback {

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        // Stop the edge-scroll self-loop: it is driven by postDelayed and
        // only stops on ACTION_UP/CANCEL. A detach mid-drag (rotation,
        // window destruction) would otherwise leave it running forever,
        // holding this view (and the whole viewModel chain) via the
        // handler's runnable and burning main-thread cycles on
        // repositionHandle.
        stopEdgeScroll()
        // Clear the fling scroll-end marker and the render-unpause callback:
        // both are postDelayed runnables that capture this view; after a
        // detach they would fire on a destroyed window (and repeated flings
        // stack up multiple copies).
        scrollEndRunnable?.let { removeCallbacks(it) }
        scrollEndRunnable = null
        pendingUnpauseRunnable?.let { removeCallbacks(it) }
        pendingUnpauseRunnable = null
        // Dismiss the floating selection UI: an action mode, the selection
        // handle popups and the magnifier all own system windows that hold
        // this view (and the whole viewModel chain) alive after the view is
        // detached — same leak class as the runnables above. The surface
        // teardown path also calls this, but a detach can happen without a
        // surface destruction (Compose replaces the view during
        // recomposition).
        selectionHandles.hideSelectionHandles()
        hideSelectionMenu()
        try {
            magnifier?.dismiss()
        } catch (exception: Exception) {
            LogUtil.w(TAG, "onDetachedFromWindow: magnifier dismiss failed", exception)
        }
        magnifier = null
        // Stop the PtyWriter sender thread: the view is being destroyed
        // (activity recreation, back press) and a fresh TerminalSurface will
        // build a new InputBatchBuffer. Without this, every recreation leaks
        // a daemon thread that pins the whole view chain via its sink closure.
        inputBatchBuffer.close()
    }

    private fun stopEdgeScroll() {
        edgeScrollRunning = false
        edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
    }

    fun setDimensions(rows: Int, cols: Int) = resizeManager.setDimensions(rows, cols)

    /** Forward: selection handle popups live in [SelectionHandles]. */
    fun showSelectionHandles(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        themeFgColor: Int,
    ) = selectionHandles.showSelectionHandles(startRow, startCol, endRow, endCol, themeFgColor)

    /** Forward: selection handle popups live in [SelectionHandles]. */
    fun hideSelectionHandles() = selectionHandles.hideSelectionHandles()

    private var selectionActionMode: android.view.ActionMode? = null

    /**
     * System selection toolbar (ActionMode).
     *
     * Reference: ghostty-android TerminalView.java:1423-1505 and termux-app
     * TextSelectionCursorController.java:110-215. Both use
     * ActionMode.TYPE_FLOATING so the toolbar floats near the selection
     * instead of pinning to the top. The full anchoring (Callback2 +
     * onGetContentRect — exact selection rect, single-line narrowing,
     * handle-height offset) is deferred: Kotlin requires
     * `object : ActionMode.Callback2()` (constructor parens — Callback2 is an
     * abstract class, not an interface) and the system positions TYPE_FLOATING
     * reasonably even without it. Menu order/content mirrors termux
     * (COPY/SELECT_ALL/PASTE; PASTE enabled only when the clipboard has text —
     * ghostty-android clipboardHasText() metadata check avoids the clipboard
     * access toast).
     */
    private inner class SelectionActionCallback : android.view.ActionMode.Callback2() {
            override fun onCreateActionMode(
                mode: android.view.ActionMode,
                menu: android.view.Menu,
            ): Boolean {
                val selection = viewModel?.state?.value?.selection ?: return false
                val pasteOnly = selection.pasteOnly
                if (pasteOnly) {
                    menu.add(
                        android.view.Menu.NONE,
                        MENU_ACTION_PASTE,
                        0,
                        R.string.paste,
                    ).setShowAsAction(android.view.MenuItem.SHOW_AS_ACTION_ALWAYS)
                } else {
                    menu.add(
                        android.view.Menu.NONE,
                        MENU_ACTION_COPY,
                        0,
                        R.string.copy,
                    ).setShowAsAction(android.view.MenuItem.SHOW_AS_ACTION_ALWAYS)
                    menu.add(
                        android.view.Menu.NONE,
                        MENU_ACTION_SELECT_ALL,
                        1,
                        R.string.select_all,
                    ).setShowAsAction(android.view.MenuItem.SHOW_AS_ACTION_ALWAYS)
                    menu.add(
                        android.view.Menu.NONE,
                        MENU_ACTION_PASTE,
                        2,
                        R.string.paste,
                    ).setShowAsAction(android.view.MenuItem.SHOW_AS_ACTION_ALWAYS)
                }
                return true
            }

            override fun onPrepareActionMode(
                mode: android.view.ActionMode,
                menu: android.view.Menu,
            ): Boolean = false

            /**
             * Anchor the floating toolbar exactly over the selection
             * (termux-app TextSelectionCursorController.java:194-213).
             *
             * Out-rect = selection rect expanded one cell each side
             * vertically (so the toolbar never covers the selected text)
             * and offset by the handle height. Bottom-clamped to the view.
             */
            override fun onGetContentRect(
                mode: android.view.ActionMode,
                view: android.view.View,
                outRect: android.graphics.Rect,
            ) {
                val selection = viewModel?.state?.value?.selection
                val start = selection?.start
                val end = selection?.end
                val cw = viewModel?.runtime?.cellWidth ?: 0f
                val ch = viewModel?.runtime?.cellHeight ?: 0f
                if (start == null || end == null || cw <= 0f || ch <= 0f) {
                    // No geometry: default to the whole view (system default).
                    outRect.set(0, 0, view.width, view.height)
                    return
                }
                val (topRow, bottomRow) =
                    if (start.row <= end.row) start.row to end.row else end.row to start.row
                val (leftCol, rightCol) =
                    if (start.row < end.row || start.col <= end.col) start.col to end.col else end.col to start.col
                // Selection rows are grid coordinates (scrollback rows).
                // Viewport top grid row = scrollbackLength - scrollOffset,
                // so the visible row = gridRow - (scrollbackLength - scrollOffset).
                val viewportTopGrid = (viewModel?.runtime?.bridge()?.scrollbackLength() ?: 0) - scrollOffset
                val visibleTop = (topRow - viewportTopGrid) * ch
                val visibleBottom = ((bottomRow + 1) - viewportTopGrid) * ch
                val left = leftCol * cw
                val right = (rightCol + 1) * cw
                // One cell of breathing room above/below + handle-height
                // offset (termux :203-206).
                val top = (visibleTop - ch) + HANDLE_HEIGHT_OFFSET
                val bottom = visibleBottom + ch + HANDLE_HEIGHT_OFFSET
                outRect.set(
                    left.toInt(),
                    top.toInt().coerceAtLeast(0),
                    right.toInt(),
                    bottom.toInt().coerceAtMost(view.height),
                )
            }

            override fun onActionItemClicked(
                mode: android.view.ActionMode,
                item: android.view.MenuItem,
            ): Boolean {
                when (item.itemId) {
                    MENU_ACTION_COPY -> {
                        viewModel?.copySelectionToClipboard()
                        viewModel?.clearSelection()
                        mode.finish()
                    }

                    MENU_ACTION_SELECT_ALL -> {
                        viewModel?.selectAll()
                        mode.finish()
                    }

                    MENU_ACTION_PASTE -> {
                        viewModel?.pasteFromClipboard()
                        viewModel?.clearSelection()
                        mode.finish()
                    }

                    else -> return false
                }
                return true
            }

            override fun onDestroyActionMode(mode: android.view.ActionMode) {
                selectionActionMode = null
            }

        }

    /**
     * Show the selection context menu as the system [android.view.ActionMode]
     * toolbar (Termux pattern): the system positions it at the top, colors it
     * from the platform theme and renders the items — no custom popup, no
     * accent-colored text, no dividers (round-216).
     */
    fun showSelectionMenu(pasteOnly: Boolean) {
        hideSelectionMenu()
        if (!isAttachedToWindow) return
        // Round-218: TYPE_FLOATING (FloatingToolbar) does not render on the
        // API-35 emulator (verified: whole-view anchor still invisible).
        // Fall back to the default top ActionMode — the standard system
        // toolbar used by Termux; the system colors it and positions it.
        @Suppress("DEPRECATION")
        selectionActionMode = startActionMode(SelectionActionCallback())
    }

    /** Dismiss the system selection toolbar. */
    fun hideSelectionMenu() {
        selectionActionMode?.finish()
        selectionActionMode = null
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 5b: Resize/grid computation (extracted K4)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns grid/size computation: view-size → rows/cols → PTY resize and
     * swapchain reconfigure. Extracted from TerminalSurface
     * (kotlin-architecture-round2 K4). Inner class: accesses the outer
     * view's rows/cols/lastConfigured* fields directly.
     */
    inner class ResizeManager {
        internal fun applyGridResize(
            width: Int,
            height: Int,
            imeBottom: Int,
        ) {
            val cellWidth = viewModel?.runtime?.cellWidth ?: return
            val cellHeight = viewModel?.runtime?.cellHeight ?: return
            if (cellWidth <= 0f || cellHeight <= 0f) return
            val availableHeight = height - imeBottom - modifierBarHeightPx
            if (availableHeight <= 0) return
            val newCols = (width.toFloat() / cellWidth).toInt().coerceAtLeast(1)
            val newRows = (availableHeight.toFloat() / cellHeight).toInt().coerceAtLeast(1)
            Log.d(
                "TerminalSurface",
                "applyGridResize: $width x $height ime=$imeBottom cell=($cellWidth,$cellHeight) " +
                    "-> ${newRows}x$newCols (was ${rows}x$cols)",
            )
            if (newRows != rows || newCols != cols) {
                viewModel?.runtime?.resize(newRows, newCols)
                rows = newRows
                cols = newCols
            }
        }

        internal fun recomputeRowsColsImmediate(
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

        internal fun applySurfaceResize(
            width: Int,
            height: Int,
        ) {
            if (width <= 0 || height <= 0) return
            if (width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0) return
            val terminalViewModel = viewModel ?: return
            terminalViewModel.surfaceWidth = width
            terminalViewModel.surfaceHeight = height

            // Size is handed to native via attachSurface below; there is
            // no separate surface-size channel (round-204).
            applyResizeNormal(width, height, terminalViewModel)
        }

        internal fun applyResizeNormal(
            width: Int,
            height: Int,
            terminalViewModel: TerminalViewModel,
        ) {
            terminalViewModel.runtime.recomputeGrid(width, height)
            val surface = holder.surface
            if (!surface.isValid) {
                Log.w(TAG, "applySurfaceResize: surface not valid yet, deferring")
                return
            }
            terminalViewModel.currentSurface = surface
            // ADR-0007: hand the Surface to native; the renderer builds a
            // wgpu surface from it (attachWindow JNI extracts the
            // ANativeWindow inside Rust).
            terminalViewModel.runtime.attachSurface(surface, width, height)
            val runtimeState = terminalViewModel.runtime.state.value
            if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                rows = runtimeState.rows
                cols = runtimeState.cols
            } else if (!runtimeState.isRunning) {
                // start() bails out on small surfaces (split-screen, freeform,
                // foldable half-screen) and nothing retries it — the terminal
                // would stay blank forever once the window grows, because
                // surfaceChanged only resizes. Retry session creation here now
                // that the surface is valid and sized.
                Log.i(TAG, "applySurfaceResize: runtime not started, retrying default session")
                terminalViewModel.ensureDefaultSession()
            }
            lastConfiguredWidth = width
            lastConfiguredHeight = height
            // Rotation / window-size changes (without an IME event) never reach
            // runtime.resize: the only other trigger is onApplyWindowInsets.
            // Use the shared formula so both paths agree on the grid (round-112).
            // Effective only once real cell metrics arrive (Bridge.getCellWidth
            // is an ADR-0007 stub returning 0, so this is a no-op until then).
            applyGridResize(width, height, lastImeBottom)
        }

        internal fun setDimensions(
            rows: Int,
            cols: Int,
        ) {
            this@TerminalSurface.rows = rows
            this@TerminalSurface.cols = cols
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 5c: IME input connection (extracted K4)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns the IME InputConnection: composition tracking, commit/delete
     * handling and the keyboardMode-to-EditorInfo mapping. Extracted from
     * TerminalSurface (kotlin-architecture-round2 K4). The outer class
     * exposes finishComposing/restoreKeyboardFocus as thin forwards.
     */
    inner class ImeConnection {
        var currentInputConnection: InputConnection? = null

        fun createInputConnection(outAttrs: EditorInfo): InputConnection {
            val mode = viewModel?.state?.value?.keyboardMode ?: KeyboardMode.Raw
            mode.toEditorInfo(outAttrs)
            val connection =
                object : BaseInputConnection(this@TerminalSurface, true) {
                    // Tracks in-progress IME composition so deltas reconcile instead
                    // of being dropped. Modeled on Haven's WaylandDesktopView
                    // setComposingText / commitText (core/wayland/.../WaylandDesktopView.kt:296-329).
                    private var composingBuffer: String = ""

                    private fun encodeAndSend(
                        text: String,
                        ctrlActive: Boolean,
                        altActive: Boolean,
                    ) {
                        inputBatchBuffer.write(
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
                                // Composition contracted: backspace the removed
                                // characters. Count code points, not UTF-16
                                // units — an emoji in the removed suffix is one
                                // character, and counting units would emit extra
                                // backspaces that eat a committed neighbour.
                                val removed =
                                    codePointCount(composingBuffer) - codePointCount(newComposing)
                                viewModel?.writeToPty(
                                    ByteArray(removed) { BACKSPACE_BYTE },
                                )
                            }

                            else -> {
                                // Diverged: replace the whole composing run.
                                viewModel?.writeToPty(
                                    ByteArray(codePointCount(composingBuffer)) { BACKSPACE_BYTE },
                                )
                                encodeAndSend(newComposing, ctrlActive = false, altActive = false)
                            }
                        }
                        composingBuffer = newComposing
                        return true
                    }

                    override fun finishComposingText(): Boolean {
                        composingBuffer = ""
                        return true
                    }

                    override fun commitText(
                        text: CharSequence?,
                        newCursorPosition: Int,
                    ): Boolean {
                        if (isPaused || System.nanoTime() < suppressUntilNanos) {
                            composingBuffer = ""
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
                                    ByteArray(codePointCount(composingBuffer)) { BACKSPACE_BYTE },
                                )
                                encodeAndSend(committedText, ctrlActive, altActive)
                            }
                            composingBuffer = ""
                        } else {
                            encodeAndSend(committedText, ctrlActive, altActive)
                        }
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
                        // beforeLength/afterLength come from the IME (untrusted):
                        // a negative or huge value would crash with
                        // NegativeArraySizeException / OutOfMemoryError on the
                        // main thread. Clamp to the composing-buffer length when
                        // composing, otherwise to a sane single-line maximum.
                        val maxDeletes = composingBuffer.length.coerceAtLeast(MAX_SURROUNDING_DELETES)
                        val safeBefore = beforeLength.coerceIn(0, maxDeletes)
                        val safeAfter = afterLength.coerceIn(0, maxDeletes)
                        if (safeBefore > 0) {
                            // Keep composingBuffer in sync with what the PTY will
                            // contain: setComposingText's incremental logic
                            // (startsWith/append/backspace branches) assumes the
                            // buffer mirrors the committed+composing text.
                            // Otherwise IME backspace during composition deletes
                            // once here and again in setComposingText's backspace
                            // branch, eating an extra character.
                            if (composingBuffer.isNotEmpty()) {
                                // beforeLength counts code points (API 33+);
                                // drop that many from the end, walking over
                                // surrogate pairs so emoji stay aligned with the
                                // PTY content.
                                var removed = 0
                                var end = composingBuffer.length
                                while (removed < safeBefore && end > 0) {
                                    val codePoint = composingBuffer.codePointBefore(end)
                                    end -= Character.charCount(codePoint)
                                    removed++
                                }
                                composingBuffer = composingBuffer.substring(0, end)
                            }
                            val bs = ByteArray(safeBefore) { BACKSPACE_BYTE }
                            viewModel?.writeToPty(bs)
                        }
                        if (safeAfter > 0) {
                            val del = ByteArray(safeAfter) { DELETE_BYTE }
                            viewModel?.writeToPty(del)
                        }
                        return true
                    }
                }
            imeConnection.currentInputConnection = connection
            return connection
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 5d: Selection handle popups (extracted K4)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns the selection/cursor handle PopupWindows: show, reposition,
     * drag and dismiss. Extracted from TerminalSurface
     * (kotlin-architecture-round2 K4). handleDragState/HandleDrag stay on
     * the outer class (the touch path reads them).
     */
    inner class SelectionHandles {
        private var startHandlePopup: PopupWindow? = null
        private var endHandlePopup: PopupWindow? = null
        private var cursorHandlePopup: PopupWindow? = null
        private val startHandleRect = Rect()
        private val endHandleRect = Rect()

        /** Hit-test rects for the drag handles (read by onTouchEvent). */
        internal fun startHandleHitRect() = startHandleRect
        internal fun endHandleHitRect() = endHandleRect

        fun showSelectionHandles(
            startRow: Int,
            startCol: Int,
            endRow: Int,
            endCol: Int,
            themeFgColor: Int,
        ) {
            Log.d(
                "Selection",
                "showSelectionHandles: ($startRow,$startCol)-($endRow,$endCol) " +
                    "startPopup=${startHandlePopup != null} endPopup=${endHandlePopup != null} " +
                    "cursorPopup=${cursorHandlePopup != null}",
            )
            selectionHandles.hideSelectionHandles()
            if (startRow < 0 || startCol < 0 || endRow < 0 || endCol < 0) return
            // showAtLocation requires a window token; during activity-finish
            // transition frames the view may already be detached and the call
            // throws BadTokenException.
            if (!isAttachedToWindow) return

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
                    try {
                        showAtLocation(this@TerminalSurface, 0, loc[0] + startX, loc[1] + startY)
                    } catch (exception: Exception) {
                        // WindowManager.BadTokenException: the activity detached
                        // between the isAttachedToWindow check and showAtLocation
                        // (back press / rotation while a drag handle is up).
                        LogUtil.w(TAG, "showSelectionHandles: start popup show failed", exception)
                    }
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
                    try {
                        showAtLocation(this@TerminalSurface, 0, loc[0] + endX, loc[1] + endY)
                    } catch (exception: Exception) {
                        // WindowManager.BadTokenException — see start handle above.
                        LogUtil.w(TAG, "showSelectionHandles: end popup show failed", exception)
                    }
                }
            endHandleRect.set(endX, endY, endX + handleW, endY + handleH)
            endHandleRect.inset(-handleW / 4, -handleH / 4)
        }

        internal fun repositionHandle(
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
            try {
                popup?.update(popupX, popupY, -1, -1)
            } catch (exception: Exception) {
                // WindowManager.BadTokenException: the activity detached while
                // the drag handle was being repositioned (back press/rotation).
                // Same class of race as showSelectionHandles' showAtLocation.
                LogUtil.w(TAG, "repositionHandle: popup update failed", exception)
            }

            val rect = if (which == HandleDrag.START) startHandleRect else endHandleRect
            rect.set(
                adjustedX,
                clampedY,
                adjustedX + handleW,
                clampedY + handleH,
            )
            rect.inset(-handleW / 4, -handleH / 4)
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
                    try {
                        showAtLocation(this@TerminalSurface, 0, popupX, loc[1] + cursorY)
                    } catch (exception: Exception) {
                        // WindowManager.BadTokenException — activity detached
                        // between isAttachedToWindow and showAtLocation.
                        // Same guard as showSelectionHandles.
                        LogUtil.w(TAG, "showCursorHandle: popup show failed", exception)
                    }
                }
        }

        fun hideCursorHandle() {
            try {
                cursorHandlePopup?.dismiss()
            } catch (exception: Exception) {
                // PopupWindow.dismiss can throw "View not attached to window
                // manager" when the activity was torn down first (rotation/
                // finish during handle drag). Symmetric with the show guards.
                LogUtil.w(TAG, "hideCursorHandle: dismiss failed", exception)
            }
            cursorHandlePopup = null
        }

        internal fun createHandleViewWithDrawable(
            drawable: android.graphics.drawable.Drawable,
            which: HandleDrag,
        ): View = object : View(context) {
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
                        // Only latch the drag: the DOWN coordinates can be
                        // a cell below the handle's anchor (the handle window
                        // hangs below its anchor cell), so updating the
                        // selection here would extend it by a row the moment
                        // the user grabs the handle. Selection follows the
                        // finger from the first MOVE instead, computed as a
                        // delta relative to the anchor cell boundary.
                        latchDragAnchor(which)
                        // Menu hiding during drag is driven by
                        // updateSelection/updateSelectionStart setting
                        // menuDismissed=true (TerminalScreen LaunchedEffect);
                        // re-show on UP via endSelection setting it false
                        // (ghostty-android reshowToolbar pattern).
                        return true
                    }

                    MotionEvent.ACTION_MOVE -> {
                        val (gridRow, col) = dragTargetFromTouch(localX, localY)
                        if (which == HandleDrag.START) {
                            viewModel?.updateSelectionStart(gridRow, col)
                        } else {
                            viewModel?.updateSelection(gridRow, col)
                        }
                        repositionHandle(which, gridRow, col)
                        return true
                    }

                    MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                        viewModel?.endSelection()
                        handleDragState = HandleDrag.NONE
                        val selection = viewModel?.state?.value?.selection
                        if (selection?.start != null && selection?.end != null) {
                            selectionHandles.hideSelectionHandles()
                            selectionHandles.showSelectionHandles(
                                selection.start.row,
                                selection.start.col,
                                selection.end.row,
                                selection.end.col,
                                getAccentColor(),
                            )
                            // Menu re-show is driven by TerminalScreen's
                            // LaunchedEffect(menuDismissed): endSelection sets
                            // menuDismissed=false → effect calls
                            // showSelectionMenu(pasteOnly) at the new range
                            // (ghostty-android reshowToolbar pattern).
                        }
                        return true
                    }
                }
                return super.onTouchEvent(event)
            }
        }

        internal fun createHandlePopup(contentView: View): PopupWindow {
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
                // touchModal must stay true (the default): with touchModal
                // false the popup window never receives touch events and the
                // handle's onTouchEvent (drag) can never fire — touches fall
                // through to the terminal and clear the selection. With
                // touchModal true + focusable false, touches inside the
                // handle are delivered to the handle view and touches outside
                // pass through without dismissing it.
            }
            popup.setContentView(contentView)
            return popup
        }

        fun hideSelectionHandles() {
            Log.d(
                "Selection",
                "hideSelectionHandles: start=${startHandlePopup != null} end=${endHandlePopup != null}",
            )
            try {
                startHandlePopup?.dismiss()
            } catch (exception: Exception) {
                LogUtil.w(TAG, "hideSelectionHandles: start dismiss failed", exception)
            }
            startHandlePopup = null
            try {
                endHandlePopup?.dismiss()
            } catch (exception: Exception) {
                LogUtil.w(TAG, "hideSelectionHandles: end dismiss failed", exception)
            }
            endHandlePopup = null
            hideCursorHandle()
        }
    }

    /**
     * Selection context menu as a [PopupWindow].
     *
     * Round-217: the previous Compose-drawn menu lived inside the same
     * window as the terminal SurfaceView, so the SurfaceView's surface
     * (which punches a hole over the whole terminal area) covered it —
     * the menu was invisible on device. PopupWindows are separate system
     * windows that always render above the SurfaceView and the
     * ModifierBar, exactly like the selection handles.
     */

    companion object {
        private const val TAG = "TerminalSurface"
        private const val MENU_ACTION_COPY = 1
        private const val MENU_ACTION_SELECT_ALL = 2
        private const val MENU_ACTION_PASTE = 3
        // Floating toolbar anchor offset below the selection top (termux
        // TextSelectionCursorController uses the handle height; a fixed
        // cell-fraction keeps the toolbar clear of the selected line).
        private const val HANDLE_HEIGHT_OFFSET = 24f
        private const val SWIPE_THRESHOLD_PIXELS = 500f
        private const val DEFAULT_ROWS = 24
        private const val DEFAULT_COLS = 80
        private const val DOUBLE_TAP_WINDOW_MS = 400L
        private const val ZOOM_THRESHOLD_LOW = 0.9f
        private const val ZOOM_THRESHOLD_HIGH = 1.1f
        private const val DRAWER_WIDTH_DP = 280

        private const val FLING_VELOCITY_DIVISOR = 100f
        private const val SUPPRESS_GRACE_PERIOD_NS = 50_000_000L
        private const val DRAWER_CLOSE_TAP_GRACE_NANOS = 350_000_000L // 350ms close animation
        private const val FLING_MAX_LINES = 50
        private const val SCROLL_END_DELAY_MS = 300L
        private const val SCROLLBACK_QUERY_THROTTLE_NANOS = 100_000_000L // 10 Hz

        private const val FALLBACK_CELL_WIDTH = 8f
        private const val FALLBACK_CELL_HEIGHT = 16f
        private const val BACKSPACE_BYTE = 0x08.toByte()
        private const val DELETE_BYTE = 0x7F.toByte()

        // Upper bound for deleteSurroundingText arguments (untrusted IME
        // input). One line is more than any real IME requests at once.
        // Upper bound for deleteSurroundingText arguments (untrusted
        // IME input). 4096 covers select-all delete of a large committed
        // block (round-210 P2-10: 256 left >256-char selections
        // half-deleted) while still bounding the PTY write size.
        private const val MAX_SURROUNDING_DELETES = 4096

        /** Number of Unicode code points in [text] (surrogate-pair safe). */
        private fun codePointCount(text: String): Int = text.codePointCount(0, text.length)
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

    // Scrollback-length cache (round-209, P1-3): `scrollbackLength()` is a
    // synchronous JNI query that can block up to 500 ms when the VT thread
    // is busy parsing a large write. The gesture path calls it on every
    // MotionEvent, so it is throttled to ~10 Hz and the cached value is
    // used in between — prevents UI-thread jank / ANR during scroll.
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
        bridge.setSearchHighlights(data.copyOf()) // defensive copy for JNI
        viewModel?.runtime?.forceRender()
    }

    fun clearSearchHighlights() {
        val bridge = viewModel?.runtime?.bridge() ?: return
        bridge.clearSearchHighlights()
        // Force render after clearing highlights so the inverted colors disappear
        // immediately instead of lingering for a frame.
        viewModel?.runtime?.forceRender()
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
                selectionHandles.hideSelectionHandles()
            } else {
                // Round-215: ModalNavigationDrawer's scrim click closes the
                // drawer, but during the close animation the tap can fall
                // through to the TerminalSurface — a terminal tap clears
                // the selection, so closing the drawer would silently wipe
                // an active text selection. Suppress tap-to-clear for the
                // drawer close animation duration (300ms) so the selection
                // survives the drawer's own close gesture.
                suppressUntilNanos = System.nanoTime() + DRAWER_CLOSE_TAP_GRACE_NANOS
            }
        }

    /** When true, the search bar is shown and modifier bar is hidden — touches
     *  should reach the terminal surface instead of being excluded at the bottom. */
    var searchActive: Boolean = false

    private val drawerWidthPixels: Float by lazy { DRAWER_WIDTH_DP.toFloat() * resources.displayMetrics.density }

    @Suppress("CyclomaticComplexMethod", "ComplexCondition") // Acceptable — dispatches ~15 distinct gesture/intent types

    private var cachedCellWidth: Float = FALLBACK_CELL_WIDTH
    private var cachedCellHeight: Float = FALLBACK_CELL_HEIGHT

    val cellWidth: Float
        get() {
            // Font-metric cell width (runtime.cellWidth = native font
            // pipeline cell_metrics × density). The surface÷cols ratio is
            // NOT used for hit-testing: the renderer draws glyphs at the
            // font cell size and stretches the grid, so dividing the
            // surface by the grid would double-count the stretch and land
            // long-presses on the wrong cells (round-214 regression, fixed
            // again here: the surface÷grid ratio self-amplifies — a wider
            // cell shrinks the grid, which widens the cell further).
            val viewModelCellWidth = viewModel?.runtime?.cellWidth ?: 0f
            if (viewModelCellWidth > 0f) {
                cachedCellWidth = viewModelCellWidth
                return viewModelCellWidth
            }
            return cachedCellWidth
        }

    val cellHeight: Float
        get() {
            // Font-metric cell height — see cellWidth above.
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
    private var scrollEndRunnable: Runnable? = null

    @JvmField
    var isAfterLongPress = false

    var lastTapTime = 0L

    @JvmField
    var scaleFactor = 1.0f

    internal enum class HandleDrag { NONE, START, END }

    private var handleDragState = HandleDrag.NONE
    private var selectionHandleWidth = 0

    // Drag anchor: the cell boundary the grabbed handle was pinned to when
    // the drag started (grid coordinates). Drag deltas are computed relative
    // to this anchor because the handle window hangs below its anchor cell —
    // raw touch pixels would resolve to the row below the boundary.
    //
    // Reference: termlib applyHandleDrag (Terminal.kt:1899-1935) uses the same
    // anchor semantics plus a CROSSING FLIP — when the dragged handle crosses
    // the stationary one, ownership swaps and the stationary handle returns to
    // its pre-cross position. torvox currently only coerceIn-clamps (no flip);
    // mirrored as P1 gap (round-217, research-termlib.md deep-v4).
    private var dragAnchorRow = 0
    private var dragAnchorCol = 0

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
        val gridRow = currentScrollbackLength() - scrollOffset + row
        return gridRow to col
    }

    private fun latchDragAnchor(which: HandleDrag) {
        handleDragState = which
        val selection = viewModel?.state?.value?.selection
        if (which == HandleDrag.START) {
            dragAnchorRow = selection?.start?.row ?: 0
            dragAnchorCol = selection?.start?.col ?: 0
        } else {
            dragAnchorRow = selection?.end?.row ?: 0
            dragAnchorCol = selection?.end?.col ?: 0
        }
    }
    private var longPressDragging = false
    private var longPressStartX = 0f
    private var longPressStartY = 0f

    private val clipboardAccess = ClipboardAccess(context, tag = "Surface")
    private val resizeManager = ResizeManager()
    private val imeConnection = ImeConnection()
    private val selectionHandles = SelectionHandles()
    private val clipboardPaster = ClipboardPaster(clipboardAccess)

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
                // scroll: a full-viewport swipe (distance = cellHeight ×
                // viewport rows) then scrolls exactly one viewport. The
                // previous scale²/4 formula scrolled 6–10× too fast.
                val rawAmount = (distanceY / cellHeight.coerceAtLeast(1f)).toInt()
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

                // velocityY is positive when the finger moves DOWN
                // (standard gesture coordinates), which must scroll toward
                // the newest content (offset decreases); onScroll's
                // distanceY has the opposite sign convention (positive =
                // finger moved UP = older content). Negate here so fling
                // and drag scroll in the same direction (round-211,
                // emulator-verified: fast downward flings jumped to older
                // history while slow drags scrolled correctly).
                val flingAmount = (-velocityY / FLING_VELOCITY_DIVISOR).toInt().coerceIn(-FLING_MAX_LINES, FLING_MAX_LINES)
                val newOffset = (scrollOffset + flingAmount).coerceIn(0, scrollbackLen)
                if (newOffset != scrollOffset) {
                    scrollOffset = newOffset
                    onScrollChanged?.invoke(scrollOffset)
                }
                scrollEndRunnable?.let { removeCallbacks(it) }
                scrollEndRunnable =
                    Runnable {
                        isScrolling = false
                        onScrollingStateChanged?.invoke(false)
                    }.also { postDelayed(it, SCROLL_END_DELAY_MS) }
                return true
            }

            override fun onSingleTapUp(event: MotionEvent): Boolean {
                if (isAfterLongPress) {
                    isAfterLongPress = false
                    longPressDragging = false
                    return true
                }
                // Round-215: the drawer close animation lets a scrim tap
                // fall through to the surface; do not treat it as a
                // terminal tap (which would clear the selection).
                if (System.nanoTime() < suppressUntilNanos) {
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
                    selectionHandles.hideSelectionHandles()
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
                    val gridRow = currentScrollbackLength() - scrollOffset + row
                    viewModel?.setSelectionMode(SelectionMode.Line)
                    viewModel?.startSelection(gridRow, 0)
                    val bridge = viewModel?.runtime?.bridge()
                    val line = bridge?.scrollbackLine(gridRow) ?: ""
                    viewModel?.updateSelection(gridRow, line.length.coerceAtLeast(0))
                    viewModel?.endSelection()
                    selectionHandles.showSelectionHandles(gridRow, 0, gridRow, line.length.coerceAtLeast(0), getAccentColor())
                } else {
                    startSelectionAt(event, expandToWord = true)
                    val currentSelection = viewModel?.state?.value?.selection
                    if (currentSelection?.active == true && currentSelection.start != null && currentSelection.end != null) {
                        selectionHandles.showSelectionHandles(
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
        // Reference (ghostty-android TerminalView.java:1085-1100):
        // ghostty-android uses tapCount (double-tap = word, triple-tap = line)
        // instead of long-press for word selection.  Our long-press → word
        // selection via SelectionExpander is equivalent but different UX.
        // ghostty-android also disables GestureDetector's built-in double-tap
        // detection (setOnDoubleTapListener(null)) so onSingleTapUp fires for
        // every tap and handleTap() counts them — more responsive than the
        // default 300ms+ double-tap timeout.
        if (scaleFactor < ZOOM_THRESHOLD_LOW || scaleFactor > ZOOM_THRESHOLD_HIGH) return
        isAfterLongPress = true

        @Suppress("DEPRECATION")
        performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS)

        selectionHandles.hideSelectionHandles()

        val bridge = viewModel?.runtime?.bridge()
        val scrollbackLength = bridge?.scrollbackLength() ?: 0
        val col = (x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
        val row = (y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
        val gridRow = (scrollbackLength - scrollOffset + row)

        // Always attempt smart word selection on long-press. The isCellEmpty
        // check is unreliable because the GPU render path (CellData) and the
        // query path (grid_ref) use different data sources. If the cell is
        // genuinely empty, SelectionExpander will return a zero-width range
        // and we fall through to the single-cell invert + paste menu.
        val line = bridge?.scrollbackLine(gridRow)
        val hasText = line != null && col < (line?.length ?: 0) && line?.getOrNull(col)?.isWhitespace() != true
        // Blank rows (scrollbackLine == null) are paste-only targets: there
        // is nothing to select, so long-press inverts the single cell and
        // offers Paste (round-215).
        val isOnWhitespace =
            line == null || (col < (line?.length ?: 0) && line?.getOrNull(col)?.isWhitespace() == true)

        if (isOnWhitespace) {
            viewModel?.setSelectionMode(SelectionMode.Word)
            viewModel?.startSelection(gridRow, col, TouchClass.Whitespace)
            viewModel?.endSelection()

            Log.d(
                "Selection",
                "LONG_PRESS whitespace: row=$row col=$col gridRow=$gridRow " +
                    "mode=Word menu=PASTE_ONLY",
            )
        } else {
            val bounds =
                viewModel?.runtime?.expandAndSetSelection(
                    row = gridRow,
                    col = col,
                    mode = 1, // Word mode — smart word/URL boundary detection
                )

            val startRow: Int
            val startCol: Int
            val endRow: Int
            val endCol: Int

            if (bounds != null) {
                val (start, end) = bounds
                startRow = start.first
                startCol = start.second
                endRow = end.first
                endCol = end.second
                viewModel?.setSelectionMode(SelectionMode.Semantic)
            } else {
                startRow = row
                startCol = col
                endRow = row
                endCol = col
            }

            Log.d(
                "Selection",
                "LONG_PRESS text: tapRow=$row tapCol=$col gridRow=$gridRow " +
                    "expanded start=($startRow,$startCol) end=($endRow,$endCol) " +
                    "mode=Semantic menu=FULL cellW=$cellWidth cellH=$cellHeight rows=$rows cols=$cols",
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
        // SurfaceView punches a hole in the window; the terminal content is
        // drawn by the native renderer into the Surface, everything else
        // (ModifierBar, overlays) stays in the normal view hierarchy.
        holder.setFormat(android.graphics.PixelFormat.RGBA_8888)
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
                            // Top viewport row in grid coordinates.
                            val gridRow = scrollbackLen - newOffset
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
                            // Bottom viewport row in grid coordinates.
                            val gridRow = currentScrollbackLength() - newOffset + rows - 1
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
                    selectionHandles.repositionHandle(HandleDrag.START, selection.start.row, selection.start.col)
                    selectionHandles.repositionHandle(HandleDrag.END, selection.end.row, selection.end.col)
                }
                if (edgeScrollRunning) {
                    edgeScrollHandler.postDelayed(edgeScrollRunnable, EDGE_SCROLL_INTERVAL_MS)
                }
            }
    }

    private var keyboardRequested = false

    fun finishComposing() {
        imeConnection.currentInputConnection?.let { ic ->
            ic.finishComposingText()
        }
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
            imeConnection.currentInputConnection?.let { ic ->
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
            // Inert while runtime.cellWidth/cellHeight are 0 (Bridge
            // getCellWidth is an ADR-0007 stub); activates with real cell
            // metrics (round-113).
            resizeManager.applyGridResize(width, height, imeBottom)
        }
        return result
    }

    /**
     * Compute the grid from the window size and the current IME inset, then
     * align the PTY. Single formula shared by the insets path and
     * applySurfaceResize so both can never diverge: rows exclude the IME
     * inset and the ModifierBar overlay (round-112).
     */

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

    private fun currentScrollbackLength(): Int {
        val now = System.nanoTime()
        if (now - lastScrollbackQueryNanos < SCROLLBACK_QUERY_THROTTLE_NANOS) {
            return cachedScrollbackLength
        }
        lastScrollbackQueryNanos = now
        val viewModel = viewModel ?: return cachedScrollbackLength
        val bridge = viewModel.runtime.bridge() ?: return cachedScrollbackLength
        cachedScrollbackLength =
            try {
                bridge.scrollbackLength()
            } catch (error: Exception) {
                LogUtil.e(TAG, "scrollbackLength query failed", error)
                cachedScrollbackLength
            }
        return cachedScrollbackLength
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

    /** Reset the local scroll offset to the session's offset (round-209
     *  P2-8): called on session switch so selection coordinate math does
     *  not use the previous session's offset. */
    fun resetScrollOffset() {
        val sessionOffset = viewModel?.runtime?.activeSessionScrollOffset() ?: 0
        if (scrollOffset != sessionOffset) {
            scrollOffset = sessionOffset
        }
    }

    fun getScrollOffset(): Int = scrollOffset

    fun getMaxScrollOffset(): Int = currentScrollbackLength()

    fun getRows(): Int = rows

    fun getCols(): Int = cols

    private val inputBatchBuffer = InputBatchBuffer({ data -> viewModel?.writeToPty(data) })

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection = imeConnection.createInputConnection(outAttrs)

    fun pasteFromClipboardDirect() {
        clipboardPaster.pasteTo { inputBatchBuffer.write(it) }
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
            val scrollbackLength = bridge?.scrollbackLength() ?: 0
            val bounds =
                viewModel?.runtime?.expandAndSetSelection(
                    row = (scrollbackLength - scrollOffset + row),
                    col = col,
                    mode = 1,
                )
            if (bounds != null) {
                val (start, end) = bounds
                viewModel?.setSelectionMode(SelectionMode.Word)
                viewModel?.startSelection(start.first, start.second)
                viewModel?.updateSelection(end.first, end.second)
                viewModel?.endSelection()
                Log.d(
                    "Selection",
                    "DOUBLE_TAP word: tapRow=$row tapCol=$col " +
                        "expanded start=(${start.first},${start.second}) end=(${end.first},${end.second})",
                )
            } else {
                // Selection state uses grid rows (0 = top of scrollback):
                // convert the viewport row before storing so extraction and
                // handle rendering agree.
                val scrollbackLength = viewModel?.runtime?.bridge()?.scrollbackLength() ?: 0
                viewModel?.startSelection(scrollbackLength - scrollOffset + row, col)
            }
        } else {
            val scrollbackLength = viewModel?.runtime?.bridge()?.scrollbackLength() ?: 0
            viewModel?.startSelection(scrollbackLength - scrollOffset + row, col)
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
        return KeyModifiers.fromKeyEvent(event, state?.ctrlState ?: ModifierState.Off, state?.altState ?: ModifierState.Off)
    }

    override fun onKeyDown(
        keyCode: Int,
        event: KeyEvent,
    ): Boolean {
        val terminalViewModel = viewModel
        val bridge = terminalViewModel?.runtime?.bridge()
        if (bridge != null) {
            val modifiers = modifierBitmask(event)
            val action: Int = 0 // KeyEvent.ACTION_DOWN = 0
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
            val action: Int = 1 // KeyEvent.ACTION_UP = 1
            val unicodeChar = event.unicodeChar
            val unshiftedChar = event.getUnicodeChar(event.metaState and KeyEvent.META_SHIFT_MASK.inv())
            val success = bridge.processKeyEvent(keyCode, modifiers, action, unicodeChar, unshiftedChar)
            if (success) return true
        }
        // Symmetric with onKeyDown: unhandled keys fall through to the
        // system so key-up semantics (long-press repeat, system gestures)
        // are not swallowed.
        return super.onKeyUp(keyCode, event)
    }

    private fun handleKeyEvent(event: KeyEvent): Boolean = onKeyDown(event.keyCode, event)

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4: Touch event dispatch
    // ══════════════════════════════════════════════════════════════════════

    @Suppress("CyclomaticComplexMethod", "LongMethod", "NestedBlockDepth") // Acceptable — dispatches ~15 distinct gesture/intent types
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!touchEnabled) {
            scaleDetector.onTouchEvent(event)
            gestureDetector.onTouchEvent(event)
            return false
        }
        if (event.action == MotionEvent.ACTION_DOWN) {
            parent?.requestDisallowInterceptTouchEvent(true)
        }
        // Round-215: the drawer's swipe-from-edge gesture starts in the
        // screen-edge slop (~32dp). The surface must not consume those
        // touches: when drawerOpen is false the surface would otherwise
        // receive DOWN + UP without MOVEs (the drawer gesture takes the
        // moves), GestureDetector classifies that as a tap and clears the
        // selection. Edge touches always belong to the drawer gesture.
        // While the drawer is open the surface must yield ALL touches to
        // the scrim (its close gesture), otherwise requestDisallowIntercept
        // on DOWN steals the stream and the drawer can never close.
        val drawerEdgePixels = (32 * resources.displayMetrics.density).toInt()
        if (drawerOpen || event.x < drawerEdgePixels) {
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
                    if (!selectionHandles.startHandleHitRect().isEmpty() && selectionHandles.startHandleHitRect().contains(touchX.toInt(), touchY.toInt())) {
                        // Only latch the drag: the handle window hangs below
                        // its anchor cell, so updating the selection with raw
                        // touch pixels would extend it by a row the moment the
                        // user grabs the handle (round-215). The selection
                        // follows the finger from the first MOVE via
                        // dragTargetFromTouch.
                        latchDragAnchor(HandleDrag.START)
                        return true
                    } else if (!selectionHandles.endHandleHitRect().isEmpty() && selectionHandles.endHandleHitRect().contains(touchX.toInt(), touchY.toInt())) {
                        latchDragAnchor(HandleDrag.END)
                        return true
                    } else {
                        viewModel?.clearSelection()
                        selectionHandles.hideSelectionHandles()
                    }
                }
            }

            MotionEvent.ACTION_MOVE -> {
                if (isSelectingText && handleDragState != HandleDrag.NONE) {
                    val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                    val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                    currentTouchX = event.x
                    currentTouchY = event.y

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
                        val (gridRow, col) = dragTargetFromTouch(event.x, event.y)
                        if (handleDragState == HandleDrag.START) {
                            viewModel?.updateSelectionStart(gridRow, col)
                        } else {
                            viewModel?.updateSelection(gridRow, col)
                        }
                        selectionHandles.repositionHandle(handleDragState, gridRow, col)
                    }
                } else if (longPressDragging && isSelectingText) {
                    val col = (event.x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
                    val row = (event.y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
                    val gridRow = currentScrollbackLength() - scrollOffset + row
                    Log.d("Selection", "MOVE longPressDragging: row=$gridRow col=$col")
                    viewModel?.updateSelection(gridRow, col)
                    val sel = viewModel?.state?.value?.selection
                    if (sel?.start != null && sel?.end != null) {
                        // reposition, don't rebuild: showSelectionHandles
                        // dismisses and recreates 2 PopupWindows (4
                        // WindowManager IPC + allocations) — at 60-120Hz
                        // ACTION_MOVE that is a guaranteed frame-drop
                        // source. repositionHandle uses PopupWindow.update
                        // (in-process) instead.
                        selectionHandles.repositionHandle(HandleDrag.START, sel.start.row, sel.start.col)
                        selectionHandles.repositionHandle(HandleDrag.END, sel.end.row, sel.end.col)
                    }
                }
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (longPressDragging) {
                    longPressDragging = false
                    val sel = viewModel?.state?.value?.selection
                    if (sel?.start != null && sel?.end != null) {
                        viewModel?.endSelection()
                        // Paste-only selections (empty-cell / whitespace
                        // long-press) must NOT get drag handles: the handle
                        // PopupWindows would overlap the paste menu and eat
                        // its taps (round-214).
                        if (!sel.pasteOnly) {
                            selectionHandles.showSelectionHandles(sel.start.row, sel.start.col, sel.end.row, sel.end.col, getAccentColor())
                        }
                    }
                }
                edgeScrollRunning = false
                pendingEdgeScroll = 0
                edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
                if (isSelectingText && handleDragState != HandleDrag.NONE) {
                    viewModel?.endSelection()
                    val selection = viewModel?.state?.value?.selection
                    if (selection?.start != null && selection?.end != null) {
                        selectionHandles.hideSelectionHandles()
                        selectionHandles.showSelectionHandles(
                            selection.start.row,
                            selection.start.col,
                            selection.end.row,
                            selection.end.col,
                            getAccentColor(),
                        )
                        // The selection menu is rendered as a PopupWindow
                        // (system ActionMode), driven by the view-model state.
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

    // ── Surface lifecycle (SurfaceHolder.Callback; the native renderer
    // ── draws into the Surface — SurfaceView, not TextureView, because
    // ── TextureView's SurfaceTexture is consumed by the GL compositor and
    // ── blocks Vulkan dequeueBuffer on software emulators) ────────────────

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

        surfaceWidthPixels = width
        surfaceHeightPixels = height
        resizeManager.recomputeRowsColsImmediate(width, height)
        // Resize the GPU swapchain synchronously and immediately so the rendered
        // frame always matches the new view size: if the wgpu/swapchain
        // buffer stayed at the old size for even a few frames (e.g. while the
        // IME animates), the stale buffer would be non-uniformly scaled ->
        // the text would visibly stretch/compress. Immediate resize keeps
        // buffer == view at all times, eliminating the artifact.
        resizeManager.applySurfaceResize(width, height)
    }

    /**
     * Reconfigure the native (wgpu) surface + grid to [width]x[height] right now.
     * Idempotent: a no-op when the size already matches the last configured size.
     * Must run on the main thread (holds the bridge surface lock while the render
     * thread may briefly contend, but never deadlocks).
     */

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 5: Surface lifecycle (ResizeManager owns grid/size)
    // ══════════════════════════════════════════════════════════════════════

    override fun surfaceCreated(holder: SurfaceHolder) {
        val surface = holder.surface
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
                // ADR-0007: re-attach the (recreated) surface; the renderer
                // rebuilds its wgpu surface from it.
                terminalViewModel.runtime.attachSurface(surface, width, height)
                terminalViewModel.runtime.recomputeGrid(width, height)
                terminalViewModel.runtime.resumeRendering()
                val runtimeState = terminalViewModel.runtime.state.value
                if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                    rows = runtimeState.rows
                    cols = runtimeState.cols
                }
                lastConfiguredWidth = width
                lastConfiguredHeight = height
            }
        }
    }

    override fun surfaceChanged(
        holder: SurfaceHolder,
        format: Int,
        width: Int,
        height: Int,
    ) {
        surfaceWidthPixels = width
        surfaceHeightPixels = height
        resizeManager.recomputeRowsColsImmediate(width, height)
        // Always resize the swapchain when the surface size changes,
        // including IME show/hide (height-only changes). The old approach
        // of skipping height-only changes caused text stretch/compression
        // because the GPU rendered into a stale-sized buffer while the
        // TextureView had already resized, producing non-uniform scaling.
        resizeManager.applySurfaceResize(width, height)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        viewModel?.runtime?.onSurfaceDestroyed()
        viewModel?.runtime?.releaseAllGpuSurfaces()
        // Dismiss selection/cursor handle popups: they hold an Activity
        // context and a dismissed-but-showing popup triggers StrictMode
        // activity leaks (and a BadTokenException crash on rotation when
        // the activity is being destroyed).
        selectionHandles.hideSelectionHandles()
        // Stop the edge-scroll loop: without this the self-reposting
        // runnable keeps firing every 50ms after destroy, touching a
        // cleared ViewModel and dismissed popups.
        edgeScrollRunning = false
        edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
        // Release the Android Surface only after the render thread has been
        // joined (pauseRendering runs on the surface-transition executor and
        // its join can take up to 1s per session). Releasing the ANativeWindow
        // while the render thread may still be inside native render code is a
        // use-after-free; the executor ordering guarantees the join finished.
        lastConfiguredWidth = 0
        lastConfiguredHeight = 0
        viewModel?.currentSurface = null
    }
}
