// TODO(kotlin-2.4.0-false-positive): K2 smart-cast false positive, remove when upgrading Kotlin
// compiler
@file:Suppress("UNNECESSARY_SAFE_CALL")

package terminal.emulator.ui

import android.content.Context
import android.graphics.Rect
import android.graphics.drawable.Drawable
import android.os.SystemClock
import android.util.AttributeSet
import android.util.Log
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PointerIcon
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.widget.Magnifier
import android.widget.OverScroller
import android.widget.PopupWindow
import androidx.core.net.toUri
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.SelectionMode
import terminal.emulator.TerminalViewModel
import terminal.emulator.TouchClass
import terminal.emulator.bridge.Bridge
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.KeyboardMode
import terminal.emulator.input.ModifierState
import terminal.emulator.input.toEditorInfo
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.ClipboardPaster
import terminal.emulator.runtime.InputBatchBuffer
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.isWideCodePoint
import kotlin.math.floor
import kotlin.math.roundToInt

// Legacy height reserved for the ModifierBar when it lived inside the
// terminal Column (see applyGridResize). The bar is now a Compose overlay
// outside the SurfaceView's weight(1f) Box, so the terminal grid height is
// `height - imeBottom` without subtracting the bar — double subtraction
// undersized the grid by exactly the bar height (keyboard-height jump).
// 80dp kept for reference but no longer used in the hybrid pan-then-reflow
// path; applyGridResize now uses `height - imeBottom` directly.
private val modifierBarHeightPx: Int by lazy {
    android.content.res.Resources.getSystem().displayMetrics.density.let { density ->
        (80f * density + 0.5f).toInt()
    }
}

internal fun isWordChar(c: Char): Boolean = c.isLetterOrDigit() || c == '_' || c == '-' || c == '.' || c == '/'

/**
 * True when the code point occupies two terminal cells (wide char). Covers the CJK and East-Asian
 * wide ranges from Markus Kuhn's wcwidth() tables (plus emoji ranges). Split into BMP/astral halves
 * to keep the cyclomatic complexity of each helper below the detekt threshold.
 */
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
        surfaceScope?.cancel()
        surfaceScope = null
        // release the render-loop accessibility hook; the view
        // is being destroyed and the runtime should not keep calling into
        // it (activity recreation builds a fresh TerminalSurface).
        viewModel?.runtime?.onFrameRendered = null
        accessibilityDescriptionUpdater.cancel()
        // Force-hide the IME: a detach can happen during rotation or back
        // press while the soft keyboard is open.  Without this the keyboard
        // remains visible over a destroyed Activity window (stuck-keyboard
        // bug).
        try {
            val imm =
                context.getSystemService(
                    android.content.Context.INPUT_METHOD_SERVICE,
                ) as android.view.inputmethod.InputMethodManager
            imm.hideSoftInputFromWindow(windowToken, 0)
        } catch (_: Exception) {
            // View already torn down — ignore.
        }
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
        // The fling animation is vsync-paced via postOnAnimation; cancel it
        // explicitly on detach so no step fires on a destroyed window.
        stopFlingAnimation()
        pendingUnpauseRunnable?.let { removeCallbacks(it) }
        pendingUnpauseRunnable = null
        resizeDebounceRunnable?.let { removeCallbacks(it) }
        resizeDebounceRunnable = null
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

    private var selectionMenuPopup: PopupWindow? = null

    /**
     * Selection context menu as a [PopupWindow] (the same separate-system-window pattern as the
     * selection handles): the old Compose menu was covered by the SurfaceView hole and the ActionMode
     * TYPE_FLOATING toolbar does not render on the API-35 emulator ( emulator-verified: handles
     * visible, toolbar absent). A PopupWindow always renders above the SurfaceView on every platform.
     *
     * Items mirror termux: a text selection offers COPY | SELECT ALL; a paste-only (blank-cell)
     * selection offers PASTE — and PASTE only when the clipboard actually has text
     * (ClipboardAccess.hasClipboardText), so a dead PASTE action can never show (the reported "PASTE
     * 按钮始终显示" bug). Dismissal rides the existing selection-state flow: a tap outside reaches the
     * terminal, clears the selection, and TerminalScreen's LaunchedEffect calls [hideSelectionMenu].
     */
    fun showSelectionMenu(pasteOnly: Boolean) {
        hideSelectionMenu()
        if (!isAttachedToWindow) return
        val selection = viewModel?.state?.value?.selection ?: return
        if (selection.start == null || selection.end == null) return
        val pasteEnabled = clipboardAccess.hasClipboardText()
        val actions = menuActions(pasteOnly, pasteEnabled)
        if (actions.isEmpty()) return
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
        val anchor = menuAnchor(selection) ?: return
        val loc = IntArray(2)
        getLocationInWindow(loc)
        try {
            popup.showAtLocation(this@TerminalSurface, 0, loc[0] + anchor.first, loc[1] + anchor.second)
        } catch (exception: Exception) {
            // Activity detached between the check and show — same guard as the
            // selection handles; the popup never became visible.
            LogUtil.w(TAG, "showSelectionMenu: popup show failed", exception)
            return
        }
        selectionMenuPopup = popup
    }

    /** Menu items for a selection: termux semantics (COPY|SELECT ALL / PASTE-if-clipboard). */
    private fun menuActions(
        pasteOnly: Boolean,
        pasteEnabled: Boolean,
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
            add(
                context.getString(R.string.copy) to
                    {
                        viewModel?.copySelectionToClipboard()
                        viewModel?.clearSelection()
                    },
            )
            add(
                context.getString(R.string.select_all) to
                    {
                        // termux behavior: select-all keeps the menu open so the
                        // user can immediately COPY the new selection.
                        viewModel?.selectAll()
                    },
            )
        }
    }

    /** Dark pill toolbar hosting one clickable label per action. */
    private fun buildMenuBar(actions: List<Pair<String, () -> Unit>>): android.widget.LinearLayout {
        val density = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * density + 0.5f).toInt()
        val bar =
            android.widget.LinearLayout(context).apply {
                orientation = android.widget.LinearLayout.HORIZONTAL
                val bg = android.graphics.drawable.GradientDrawable()
                bg.setColor(0xEE2B2B2B.toInt())
                bg.cornerRadius = dp(8).toFloat()
                background = bg
                elevation = dp(6).toFloat()
                for ((label, action) in actions) {
                    val item =
                        android.widget.TextView(context).apply {
                            text = label
                            setTextColor(0xFFFFFFFF.toInt())
                            textSize = 14f
                            val h = dp(16)
                            setPadding(h, dp(12), h, dp(12))
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
     * Window coordinates for the menu: above the selection top edge (same viewport math as the
     * handles), clamped into the surface. Null when cell metrics are not ready yet.
     */
    private fun menuAnchor(selection: terminal.emulator.SelectionState): Pair<Int, Int>? {
        val cw = cellWidth
        val ch = cellHeight
        if (cw <= 0f || ch <= 0f) return null
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
        val (leftPx, topPx) = gridToScreen(topRow, leftCol, viewportTopGrid, cw, ch)
        val (rightPx, _) = gridToScreen(bottomRow + 1, rightCol + 1, viewportTopGrid, cw, ch)
        val density = resources.displayMetrics.density
        val dp = { v: Int -> (v * density + 0.5f).toInt() }
        // PopupWindow measures on show, so use a rough estimate (items ×
        // ~92dp) clamped to the surface.
        val estimatedWidth = width.coerceAtMost(dp(184))
        val x =
            ((leftPx + rightPx) / 2 - estimatedWidth / 2)
                .toInt()
                .coerceIn(0, (width - estimatedWidth).coerceAtLeast(0))
        val menuHeight = dp(44)
        val y = (topPx - menuHeight - dp(4)).toInt().coerceIn(0, (height - menuHeight).coerceAtLeast(0))
        return x to y
    }

    /** Dismiss the selection menu popup. */
    fun hideSelectionMenu() {
        val popup = selectionMenuPopup
        selectionMenuPopup = null
        if (popup == null) return
        try {
            popup.dismiss()
        } catch (exception: Exception) {
            // Already dismissed — log-only so the cause is never lost.
            LogUtil.w(TAG, "hideSelectionMenu: dismiss failed", exception)
        }
    }

    /** Re-show the menu at the CURRENT selection (handle drag end re-anchor). */
    private fun reshowToolbar() {
        val pasteOnly = viewModel?.state?.value?.selection?.pasteOnly ?: false
        if (selectionMenuPopup != null) {
            showSelectionMenu(pasteOnly)
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 5b: Resize/grid computation (extracted K4)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns grid/size computation: view-size → rows/cols → PTY resize and swapchain reconfigure.
     * Extracted from TerminalSurface inner class: accesses the outer view's rows/cols/lastConfigured*
     * fields directly.
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
            // Hybrid pan-then-reflow: height is the SurfaceView's layout height
            // (already excludes navigation bars; with Compose offset it does NOT
            // shrink during animation). Settled reflow shrinks the PTY grid by
            // the IME inset only — the ModifierBar is an overlay, not inside this
            // height, so do NOT subtract modifierBarHeightPx (double-subtraction bug).
            val availableHeight = (height - imeBottom).coerceAtLeast(1)
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
                // Push the pixel dimensions alongside the grid resize so the
                // PTY winsize carries real ws_xpixel/ws_ypixel: pixel-aware
                // programs (`icat`, fullscreen TUIs) read them from
                // TIOCGWINSZ and misrender when they are 0 (ghostty-android
                // pty_jni.c:84-87). availableHeight excludes the IME inset
                // and the modifier bar — exactly the grid area rows covers.
                viewModel?.runtime?.setPixelSize(width, availableHeight)
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
            if (
                width == lastConfiguredWidth && height == lastConfiguredHeight && lastConfiguredWidth != 0
            ) {
                return
            }
            val terminalViewModel = viewModel ?: return
            terminalViewModel.surfaceWidth = width
            terminalViewModel.surfaceHeight = height

            // Size is handed to native via attachSurface below; there is
            // no separate surface-size channel.
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
            // Use the shared formula so both paths agree on the grid.
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
     * Owns the IME InputConnection: composition tracking, commit/delete handling and the
     * keyboardMode-to-EditorInfo mapping. Extracted from TerminalSurface. The outer class exposes
     * finishComposing/restoreKeyboardFocus as thin forwards.
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
                    // warp WarpInputView.kt:553 commitTextSynthExpiresMs debounce + EmptyFinish
                    private var composingBuffer: String = ""
                    private var lastCommitText: String = ""
                    private var lastCommitMs: Long = 0L
                    private val commitSynthDebounceMs = 80L

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
                        // pure reconciliation (ComposingDiff),
                        // unit-tested — grow/contract/diverged in one place.
                        val edit = ComposingDiff.reconcile(composingBuffer, newComposing)
                        // Round-234 (spec cursor-rendering "composing 链路可
                        // 观测"): trace-level anchor for automated IME
                        // verification — the log sequence must match the
                        // injected composing text one-to-one. VERBOSE only:
                        // zero cost unless explicitly enabled via
                        // Debug-only: the spec requires ZERO output on
                        // release builds (R8 -dontoptimize would not strip it).
                        if (terminal.emulator.BuildConfig.DEBUG) {
                            Log.v(
                                "ComposingDiff",
                                "reconcile prev=${composingBuffer.length}ch next=$newComposing " +
                                    "bs=${edit.backspaces} app=${edit.append.length}ch",
                            )
                        }
                        if (edit.backspaces > 0) {
                            viewModel?.writeToPty(
                                ByteArray(edit.backspaces) { BACKSPACE_BYTE },
                            )
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

                        // Synthetic commit debounce (warp 553): Gboard may fire commitText twice
                        // for same text within 80ms; drop duplicate to avoid double send.
                        val nowMs = android.os.SystemClock.uptimeMillis()
                        if (
                            composingBuffer.isEmpty() &&
                            committedText == lastCommitText &&
                            nowMs - lastCommitMs < commitSynthDebounceMs
                        ) {
                            return true
                        }
                        if (composingBuffer.isNotEmpty()) {
                            if (committedText == composingBuffer) {
                                // Already forwarded via composing deltas; do not resend.
                            } else {
                                val clear = ComposingDiff.reconcile(composingBuffer, "")
                                terminalViewModel?.writeToPty(
                                    ByteArray(clear.backspaces) { BACKSPACE_BYTE },
                                )
                                encodeAndSend(committedText, ctrlActive, altActive)
                            }
                            composingBuffer = ""
                        } else {
                            encodeAndSend(committedText, ctrlActive, altActive)
                        }
                        lastCommitText = committedText
                        lastCommitMs = nowMs
                        // EmptyFinish: when committing from empty composing, clear any stale state
                        // to prevent Gboard double-submit on next composition start.
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
     * Owns the selection-handle overlay ( redesign, spec text-selection "手柄窗口生命周期单一 owner"): ONE
     * full-surface TYPE_APPLICATION_SUB_PANEL window hosts both teardrop handles.
     *
     * Why one overlay instead of two WRAP_CONTENT popups:
     * - dragging repositions a handle by changing View translationX/Y + invalidating inside the
     *   overlay — ZERO WindowManager IPC per frame (the old dual-popup design issued 2
     *   PopupWindow.update binder transactions per ACTION_MOVE);
     * - one window created/dismissed as a unit cannot leak orphaned handles (the old
     *   dismiss-exception path left windows on screen — reported as "multiple pointers that never
     *   disappear");
     * - [HandleOverlayLayout.dispatchTouchEvent] routes handle touches to the drag logic and forwards
     *   everything else to the terminal surface (termux TextSelectionPopupView pattern).
     *
     * Drag state (handleDragState/HandleDrag/dragPointerId) stays on the outer class — the touch path
     * and edge-scroll runnable read them.
     */
    inner class SelectionHandles {
        private var overlayPopup: PopupWindow? = null
        private var overlayContent: HandleOverlayLayout? = null
        private val startHandleRect = Rect()
        private val endHandleRect = Rect()

        /** Hit-test rects for the drag handles (surface coordinates). */
        internal fun startHandleHitRect() = startHandleRect

        internal fun endHandleHitRect() = endHandleRect

        /**
         * System Material selection handle: resolve the platform theme attribute
         * (android.R.attr.textSelectHandleLeft/Right) so the handle shape is the framework's teardrop,
         * not a custom vector. Falls back to the bundled teardrop drawables when the theme attribute is
         * absent (fixes the "handles render as squares" regression caused by the old stretched 132x66
         * vectors).
         */
        internal fun resolveSelectionHandleDrawable(
            left: Boolean,
        ): android.graphics.drawable.Drawable? {
            val attr =
                intArrayOf(
                    if (left) {
                        android.R.attr.textSelectHandleLeft
                    } else {
                        android.R.attr.textSelectHandleRight
                    },
                )
            val typedArray = context.theme.obtainStyledAttributes(attr)
            val themed =
                try {
                    typedArray.getDrawable(0)
                } catch (exception: Exception) {
                    // Malformed theme / resource shadowing: fall through to the
                    // bundled teardrop instead of dropping the handle entirely.
                    LogUtil.w(TAG, "resolveSelectionHandleDrawable: theme attr failed", exception)
                    null
                } finally {
                    typedArray.recycle()
                }
            return themed
                ?: runCatching {
                    androidx.core.content.ContextCompat.getDrawable(
                        context,
                        if (left) {
                            R.drawable.text_select_handle_left_material
                        } else {
                            R.drawable.text_select_handle_right_material
                        },
                    )
                }
                    .getOrNull()
        }

        fun showSelectionHandles(
            startRow: Int,
            startCol: Int,
            endRow: Int,
            endCol: Int,
            themeFgColor: Int,
        ) {
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
            // showAtLocation requires a window token; during activity-finish
            // transition frames the view may already be detached and the call
            // throws BadTokenException.
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
                    // focusable=false keeps keyboard input flowing to the
                    // terminal while the overlay consumes touches in its
                    // bounds (dispatchTouchEvent routes them).
                    isFocusable = false
                    isOutsideTouchable = false
                }
            try {
                popup.showAtLocation(this@TerminalSurface, 0, loc[0], loc[1])
            } catch (exception: Exception) {
                // WindowManager.BadTokenException: the activity detached
                // between the isAttachedToWindow check and showAtLocation.
                LogUtil.w(TAG, "showSelectionHandles: overlay show failed", exception)
                overlayContent = null
                return
            }
            overlayPopup = popup
            positionAllHandles(startRow, startCol, endRow, endCol)
        }

        /**
         * Move one dragged handle to its anchor cell. Pure in-process view updates: translationX/Y +
         * invalidate, NO WindowManager IPC (spec text-selection "拖拽流畅性" constraint a).
         */
        internal fun repositionHandle(
            which: HandleDrag,
            row: Int,
            col: Int,
        ) {
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

        private fun positionAllHandles(
            startRow: Int,
            startCol: Int,
            endRow: Int,
            endCol: Int,
        ) {
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

        /**
         * Anchor math shared by both positioning paths (termux hotspot): START hangs below-left of its
         * cell corner, END below-right.
         */
        private fun updateHitRect(
            which: HandleDrag,
            anchorX: Int,
            anchorY: Int,
        ) {
            val handleW = selectionHandleWidth
            if (handleW == 0) return
            val content = overlayContent ?: return
            val handleH = content.handleHeight
            val x =
                (anchorX - (if (which == HandleDrag.START) (handleW * 3) / 4 else handleW / 4)).coerceIn(
                    0,
                    (width - handleW).coerceAtLeast(0),
                )
            val y = anchorY.coerceIn(0, (height - handleH).coerceAtLeast(0))
            val rect = if (which == HandleDrag.START) startHandleRect else endHandleRect
            rect.set(x, y, x + handleW, y + handleH)
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
                // Already removed by dismiss — harmlessly ignored.
            }
        }

        /**
         * Full-surface transparent container hosting both teardrop handles. Positions handles purely
         * with translationX/Y. Touch routing:
         * - DOWN inside an expanded handle hit rect latches that handle's drag (per-handle lock; the
         *   OTHER handle's stream cannot steal it — root cause C3 fix),
         * - every other event stream is forwarded untouched to the terminal surface so tap/swipe/pinch
         *   gestures behave exactly as without the overlay.
         */
        private inner class HandleOverlayLayout(
            private val leftDrawable: android.graphics.drawable.Drawable,
            private val rightDrawable: android.graphics.drawable.Drawable,
            val handleWidth: Int,
            val handleHeight: Int,
        ) : android.widget.FrameLayout(context) {
            private val startView = HandleView(leftDrawable)
            private val endView = HandleView(rightDrawable)

            /** Which handle this layout's active drag belongs to, if any. */
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
                val p = popupDeferredDismiss
                popupDeferredDismiss = null
                return p
            }

            init {
                addView(startView, LayoutParams(handleWidth, handleHeight))
                addView(endView, LayoutParams(handleWidth, handleHeight))
            }

            fun position(
                which: HandleDrag,
                anchorX: Int,
                anchorY: Int,
            ) {
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

                // ACTION_POINTER_UP included: when the OWNING finger lifts
                // while a second finger stays down, the stream delivers
                // POINTER_UP (not UP) — missing it here left drags latched
                // forever with edge-scroll possibly self-running
                // ( review-1 BLOCKING finding).
                MotionEvent.ACTION_UP,
                MotionEvent.ACTION_POINTER_UP,
                MotionEvent.ACTION_CANCEL,
                -> routeStreamEnd(event)

                // Other pointer events (POINTER_DOWN etc.) are swallowed:
                // a second finger must never open a second stream while a
                // drag or forwarding stream is owned.
                else -> true
            }

            /** DOWN: latch a handle drag or start forwarding a terminal stream. */
            private fun routeDown(event: MotionEvent): Boolean {
                val x = event.x.toInt()
                val y = event.y.toInt()
                val which = handleAt(x, y)
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

            private fun handleAt(x: Int, y: Int): HandleDrag? = when {
                !startHandleRect.isEmpty() && startHandleRect.contains(x, y) -> HandleDrag.START
                !endHandleRect.isEmpty() && endHandleRect.contains(x, y) -> HandleDrag.END
                else -> null
            }

            private fun lockedIndex(event: MotionEvent): Int = dragPointerLocked?.let { event.findPointerIndex(it) } ?: -1

            /** MOVE: drive the owned drag; swallow stray pointers; else forward. */
            private fun routeMove(event: MotionEvent): Boolean {
                if (dragOwner != null) {
                    val lockedIdx = lockedIndex(event)
                    if (lockedIdx >= 0) {
                        driveHandleDragMove(event.getX(lockedIdx), event.getY(lockedIdx))
                    }
                    // Locked finger gone: swallow stray moves from any
                    // other pointer (spec 多指防漂移).
                    return true
                }
                if (streamForwarding) {
                    return this@TerminalSurface.dispatchTouchEvent(event)
                }
                return true
            }

            /** UP/CANCEL: end the owned drag once the owning pointer lifts; else forward. */
            private fun routeStreamEnd(event: MotionEvent): Boolean {
                if (dragOwner != null) {
                    // End the drag only when the pointer that LIFTED is the
                    // locked owner (review-2): ACTION_POINTER_UP carries ALL
                    // still-down pointers, so a lockedIndex>=0 check alone
                    // would end the drag when a SECOND finger lifts while the
                    // owner keeps dragging.
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

            private inner class HandleView(
                private val drawable: android.graphics.drawable.Drawable,
            ) : View(context) {
                override fun onMeasure(
                    widthMeasureSpec: Int,
                    heightMeasureSpec: Int,
                ) {
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
        private const val WIDE_CHAR_CACHE_TTL_MS = 500L

        private const val SWIPE_THRESHOLD_PIXELS = 500f
        private const val DEFAULT_ROWS = 24
        private const val DEFAULT_COLS = 80
        private const val DOUBLE_TAP_WINDOW_MS = 400L
        private const val ZOOM_THRESHOLD_LOW = 0.9f
        private const val ZOOM_THRESHOLD_HIGH = 1.1f

        // ⑥ pinch zoom: preview bounds match TerminalScreen's FONT_SIZE
        // clamp; the preview rate is a few updates per second so sculpting
        // feels smooth without reflowing ghostty per frame (41ms frame
        // baseline on the emulator).
        private const val ZOOM_FONT_SIZE_MIN_SP = 14f
        private const val ZOOM_FONT_SIZE_MAX_SP = 48f
        private const val ZOOM_FONT_SIZE_EPSILON_SP = 0.05f
        private const val ZOOM_PREVIEW_INTERVAL_NANOS = 60_000_000L // 60ms
        private const val DRAWER_WIDTH_DP = 280

        private const val SUPPRESS_GRACE_PERIOD_NS = 50_000_000L
        private const val DRAWER_CLOSE_TAP_GRACE_NANOS = 350_000_000L // 350ms close animation
        private const val IME_RESIZE_DEBOUNCE_MS = 48L // 3×16ms settle, spec ime-translation
        private const val SCROLLBACK_QUERY_THROTTLE_NANOS = 100_000_000L // 10 Hz

        private const val FALLBACK_CELL_WIDTH = 8f
        private const val FALLBACK_CELL_HEIGHT = 16f
        private const val BACKSPACE_BYTE = 0x08.toByte()
        private const val DELETE_BYTE = 0x7F.toByte()

        // Upper bound for deleteSurroundingText arguments (untrusted IME
        // input). One line is more than any real IME requests at once.
        // Upper bound for deleteSurroundingText arguments (untrusted
        // IME input). 4096 covers select-all delete of a large committed
        // block: 256 left >256-char selections
        // half-deleted) while still bounding the PTY write size.
        private const val MAX_SURROUNDING_DELETES = 4096

        /** Number of Unicode code points in [text] (surrogate-pair safe). */
        private fun codePointCount(text: String): Int = text.codePointCount(0, text.length)

        private const val EDGE_SCROLL_INTERVAL_MS = 30L
        private const val ACCESSIBILITY_DESCRIPTION_DEBOUNCE_MILLIS = 500L
        private const val ACCESSIBILITY_SCROLLBACK_QUERY_INTERVAL_NANOS = 250_000_000L // 4 Hz

        // Custom accessibility action ids start at 0x1000; the
        // ACTION_CUSTOM_ACTION constant was removed in API 37.
        private const val ACCESSIBILITY_CUSTOM_ACTION_BASE = 0x1000
    }

    private fun getAccentColor(): Int = viewModel?.runtime?.accentColor ?: 0xFF2196F3.toInt()

    private var viewModel: TerminalViewModel? = null
    private var shortcutHandler: terminal.emulator.shortcut.KeyShortcutHandler? = null
    private var surfaceScope: kotlinx.coroutines.CoroutineScope? = null

    // accessibility integration — the SurfaceView is self-drawn
    // with no text nodes, so the visible terminal lines are surfaced via a
    // dynamic contentDescription (debounced) plus Next/Previous line custom
    // actions. The scrollback length is queried on the render thread (never
    // on the main thread — it is a synchronous JNI call) and throttled to
    // ACCESSIBILITY_SCROLLBACK_QUERY_INTERVAL_NANOS; the description itself
    // is assembled on the main thread from the cached length.
    private val accessibilityMainHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private val accessibilityLineProvider = AccessibilityLineProvider { row ->
        viewModel?.runtime?.bridge()?.scrollbackLine(row)
    }
    private val accessibilityNavigator = AccessibilityLineNavigator(accessibilityLineProvider)
    private val accessibilityDescriptionUpdater =
        DebouncedTextUpdater(
            ACCESSIBILITY_DESCRIPTION_DEBOUNCE_MILLIS,
            HandlerDebounceScheduler(accessibilityMainHandler),
        )

    @Volatile private var accessibilityDescriptionRefreshPosted = false
    private var lastAccessibilityScrollbackQueryNanos = 0L

    @Volatile private var accessibilityScrollbackLength = 0

    @Volatile private var rows: Int = DEFAULT_ROWS

    @Volatile private var cols: Int = DEFAULT_COLS
    private var surfaceWidthPixels: Int = 0
    private var surfaceHeightPixels: Int = 0
    private var isScrolling: Boolean = false
    private var scrollAccumulatorPx: Float = 0f

    @Volatile private var scrollOffset: Int = 0
    private var lastImeBottom: Int = 0

    /**
     * Whether the IME is currently visible, per the last window-insets frame. Used by the keyboard
     * toggle instead of InputMethodManager.isAcceptingText, which reports IME capability, not
     * visibility.
     */
    val imeVisible: Boolean
        get() = lastImeBottom > 0

    // IME show/hide animations fire onApplyWindowInsets with a changing
    // imeBottom every frame; each distinct value used to trigger a ghostty
    // resize (full grid reflow) immediately — visibly janky on software-GPU
    // emulators (41ms frame baseline). Coalesce to the final inset; the
    // debounce runnable is re-armed on every inset change and only fires
    // once the animation settles.
    private var resizeDebounceRunnable: Runnable? = null

    // Scrollback-length cache: `scrollbackLength()` is a
    // synchronous JNI query that can block up to 500 ms when the VT thread
    // is busy parsing a large write. The gesture path calls it on every
    // MotionEvent, so it is throttled to ~10 Hz and the cached value is
    // used in between — prevents UI-thread jank / ANR during scroll.
    @Volatile private var cachedScrollbackLength: Int = 0

    /**
     * One-entry cache for [snapToWideCharBoundary]: avoids a JNI scrollbackLine copy on every
     * handle-drag MOVE within the same row. Time-bounded so a long drag cannot serve a stale line
     * after the shell rewrote the row.
     */
    private var cachedWideCharLineRow = -1
    private var cachedWideCharLine: String? = null
    private var cachedWideCharLineAtMs = 0L
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
    var onZoomChanged: ((fontSizeSp: Float) -> Unit)? = null

    // ⑥ live zoom preview: fired a few times per second during a pinch
    // gesture with the interpolated font size; the renderer follows
    // without a grid resize. Finalizing the gesture calls onZoomChanged
    // once with the settled size (full apply + grid reflow).
    var onZoomPreview: ((fontSizeSp: Float) -> Unit)? = null

    var drawerOpen: Boolean = false
        set(value) {
            field = value
            if (value) {
                selectionHandles.hideSelectionHandles()
            } else {
                // ModalNavigationDrawer's scrim click closes the
                // drawer, but during the close animation the tap can fall
                // through to the TerminalSurface — a terminal tap clears
                // the selection, so closing the drawer would silently wipe
                // an active text selection. Suppress tap-to-clear for the
                // drawer close animation duration (300ms) so the selection
                // survives the drawer's own close gesture.
                suppressUntilNanos = System.nanoTime() + DRAWER_CLOSE_TAP_GRACE_NANOS
            }
        }

    /**
     * When true, the search bar is shown and modifier bar is hidden — touches should reach the
     * terminal surface instead of being excluded at the bottom.
     */
    var searchActive: Boolean = false

    private val drawerWidthPixels: Float by lazy {
        DRAWER_WIDTH_DP.toFloat() * resources.displayMetrics.density
    }

    @Suppress(
        "CyclomaticComplexMethod",
        "ComplexCondition",
    ) // Acceptable — dispatches ~15 distinct gesture/intent types
    private var cachedCellWidth: Float = FALLBACK_CELL_WIDTH
    private var cachedCellHeight: Float = FALLBACK_CELL_HEIGHT

    val cellWidth: Float
        get() {
            // Font-metric cell width (runtime.cellWidth = native font
            // pipeline cell_metrics × density). The surface÷cols ratio is
            // NOT used for hit-testing: the renderer draws glyphs at the
            // font cell size and stretches the grid, so dividing the
            // surface by the grid would double-count the stretch and land
            // long-presses on the wrong cells  regression, fixed
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

    @Volatile internal var isPaused = false

    @Volatile private var suppressUntilNanos = 0L

    private var pendingUnpauseRunnable: Runnable? = null

    /**
     * Fling physics (termux TerminalView:1345 pattern): an [OverScroller] drives a per-frame
     * deceleration animation instead of the old single jump of the clamped distance — the jump made
     * flings feel instant and abrupt ("上下滑动…异常并且卡顿"). [postOnAnimation] paces the steps to vsync;
     * each step only pushes the delta through onScrollChanged.
     */
    private val flingScroller = OverScroller(context)
    private val flingStepRunnable = Runnable { doFlingStep() }

    /** Advance the fling animation one vsync tick. */
    private fun doFlingStep() {
        if (!flingScroller.computeScrollOffset()) {
            finishFlingAnimation()
            return
        }
        val target = flingScroller.currY.coerceIn(0, currentScrollbackLength())
        if (target != scrollOffset) {
            scrollOffset = target
            onScrollChanged?.invoke(target)
            // Spec scroll-physics: vsync-throttled requestRender after each fling step
            // so the render thread (Mailbox) presents the newest frame without stalling.
            viewModel?.runtime?.forceRender()
        }
        postOnAnimation(flingStepRunnable)
    }

    /** Stop any in-flight fling animation (touch down, programmatic scroll). */
    private fun stopFlingAnimation() {
        if (!flingScroller.isFinished) {
            flingScroller.forceFinished(true)
            removeCallbacks(flingStepRunnable)
        }
    }

    private fun finishFlingAnimation() {
        removeCallbacks(flingStepRunnable)
        isScrolling = false
        scrollAccumulatorPx = 0f
        onScrollingStateChanged?.invoke(false)
    }

    @JvmField var isAfterLongPress = false

    var lastTapTime = 0L
    private var tapCount = 0

    @JvmField var scaleFactor = 1.0f

    // ⑥ pinch-zoom state: the gesture anchors to the rendered font size and
    // interpolates it per onScale; previews push metrics without resizing,
    // onScaleEnd finalizes once.
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
     * Pointer-id lock (issue #15 multi-pointer drift): set on ACTION_DOWN by the finger that latched
     * the drag (handle hit-test or handle popup); later ACTION_MOVE events update the selection only
     * when they carry this pointer id. Cleared on UP/CANCEL. See [acceptsDragPointer].
     */
    private var dragPointerId: Int? = null

    /**
     * uptimeMillis of the last drag end — drives the [shouldSuppressTapAfterDragEnd] menu re-show
     * guard.
     */
    private var lastHandleDragEndUptimeMs = 0L

    // Drag anchor: the cell boundary the grabbed handle was pinned to when
    // the drag started (grid coordinates). Drag deltas are computed relative
    // to this anchor because the handle window hangs below its anchor cell —
    // raw touch pixels would resolve to the row below the boundary.
    //
    // Reference: termlib applyHandleDrag (Terminal.kt:1899-1935) uses the same
    // anchor semantics plus a CROSSING FLIP — when the dragged handle crosses
    // the stationary one, ownership swaps and the stationary handle returns to
    // its pre-cross position. torvox currently only coerceIn-clamps (no flip);
    // mirrored as gap, research-termlib.md deep-v4).
    private var dragAnchorRow = 0
    private var dragAnchorCol = 0

    /**
     * Opens an OSC 8 hyperlink at viewport pixel (px, py), if any. Mirrors termux
     * TerminalView.openLinkAt: pixel→cell mapping through cellWidth/cellHeight, then queries the
     * native hyperlink URI and launches the system handler. Returns true when a link was opened.
     */
    private fun openLinkAt(px: Float, py: Float): Boolean {
        if (cellWidth <= 0f || cellHeight <= 0f) return false
        val col = pixelToCell(px, cellWidth, cols)
        val row = pixelToCell(py, cellHeight, rows)
        val gridRow = currentScrollbackLength() - scrollOffset + row
        val bridge = viewModel?.runtime?.bridge() ?: return false
        val url = bridge.hyperlinkAt(gridRow, col) ?: return false
        if (url.isBlank()) return false
        val uri =
            try {
                url.trim().toUri()
            } catch (e: IllegalArgumentException) {
                LogUtil.w(TAG, "openLinkAt: bad URI", e)
                return false
            }
        return try {
            val intent =
                android.content
                    .Intent(android.content.Intent.ACTION_VIEW, uri)
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
            context.startActivity(intent)
            true
        } catch (e: android.content.ActivityNotFoundException) {
            LogUtil.w(TAG, "openLinkAt: no handler for $uri", e)
            false
        } catch (e: SecurityException) {
            LogUtil.w(TAG, "openLinkAt: blocked for $uri", e)
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
        val gridRow = currentScrollbackLength() - scrollOffset + row
        return gridRow to snapToWideCharBoundary(gridRow, col)
    }

    /**
     * Re-hide and re-show the selection handles at the current selection range (used after ending a
     * drag so handles snap to the final cell).
     */
    private fun reshowSelectionHandles() {
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
        }
    }

    /**
     * Shared end-of-handle-drag commit — called by BOTH the overlay-owned drag path
     * ([SelectionHandles.HandleOverlayLayout]) and the legacy surface touch path: commit the final
     * selection to Compose state, arm the 300ms tap guard (termux hide-protection), re-show handles
     * snapped to the final cells + toolbar, and flush the highlight to the Rust renderer.
     */
    internal fun finishHandleDrag() {
        // Round-234 review-4 BLOCKING fix: overlay-owned drags BYPASS the
        // legacy surface UP/CANCEL branch where this cleanup used to live.
        // Without it, releasing a handle inside an edge-scroll zone left
        // edgeScrollRunning=true (50ms self-looping runnable doing JNI
        // scrollback queries and mutating the finished selection forever).
        stopEdgeScroll()
        handleDragState = HandleDrag.NONE
        dragPointerId = null
        dragWideCharCacheSession = false
        viewModel?.commitDragBounds()
        viewModel?.endSelection()
        lastHandleDragEndUptimeMs = SystemClock.uptimeMillis()
        reshowSelectionHandles()
        reshowToolbar()
        viewModel?.runtime?.forceRender()
    }

    /**
     * Handle-drag ACTION_MOVE body, extracted from onTouchEvent (detekt NestedBlockDepth):
     * pointer-locked selection update + edge-scroll driving at viewport [touchX]/[touchY] (already
     * resolved to the locked finger's slot by the caller).
     */
    private fun driveHandleDragMove(touchX: Float, touchY: Float) {
        val col = pixelToCell(touchX, cellWidth, cols)
        val row = pixelToCell(touchY, cellHeight, rows)
        currentTouchX = touchX
        currentTouchY = touchY

        // Alternate screen (TUI): edge scroll is disabled — the drag belongs
        // to the remote full-screen buffer, not local scrollback (termux
        // TextSelectionCursorController :218-337 semantics). Priority:
        // SCROLL lock > alternate-screen disable > normal scroll.
        // Uses the drag-start snapshot: MOVE frames issue zero JNI calls
        // ( spec "drag fluency" constraint b).
        val altScreenActive = dragAltScreenSnapshot
        when (edgeScrollDirection(touchY, surfaceHeightPixels.toFloat(), cellHeight)) {
            EdgeScrollDirection.UP -> {
                if (altScreenActive) {
                    stopEdgeScroll()
                } else if (!edgeScrollRunning) {
                    edgeScrollRunning = true
                    pendingEdgeScroll = 1
                    edgeScrollHandler.postDelayed(edgeScrollRunnable, EDGE_SCROLL_INTERVAL_MS)
                }
            }

            EdgeScrollDirection.DOWN -> {
                if (altScreenActive) {
                    stopEdgeScroll()
                } else if (!edgeScrollRunning) {
                    edgeScrollRunning = true
                    pendingEdgeScroll = -1
                    edgeScrollHandler.postDelayed(edgeScrollRunnable, EDGE_SCROLL_INTERVAL_MS)
                }
            }

            EdgeScrollDirection.STOP -> {
                edgeScrollRunning = false
                pendingEdgeScroll = 0
                edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
                val (gridRow, snapCol) = dragTargetFromTouch(touchX, touchY)
                // Fast drag path (): compute bounds and reposition handles
                // directly, bypassing Compose _state.update → recomposition → read-back
                // round-trip that was the main per-MOVE frame bottleneck.
                val bounds =
                    viewModel?.dragMove(
                        draggingStart = handleDragState == HandleDrag.START,
                        row = gridRow,
                        col = snapCol,
                        cachedMaxRow = (rows - 1).coerceAtLeast(0),
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
     * Move the dragged handle to the cell at [gridRow] under the current touch column, snapping
     * across wide (CJK) character boundaries.
     */
    private fun updateDragHandleForCell(gridRow: Int) {
        val curCol = (currentTouchX / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
        val snappedCol = snapToWideCharBoundary(gridRow, curCol)
        if (handleDragState == HandleDrag.START) {
            viewModel?.updateSelectionStart(gridRow, snappedCol)
        } else if (handleDragState == HandleDrag.END) {
            viewModel?.updateSelection(gridRow, snappedCol)
        }
        // Throttled native push keeps the cell-inversion highlight live during
        // edge-scroll drags (same cadence as the fast MOVE path).
        viewModel?.syncDragSelectionToNativeThrottled()
    }

    /**
     * Snap a column onto a wide-char boundary: when `col` lands on the trailing (second) half of a
     * wide char, step back one cell so the selection handle never splits a wide character in two.
     */
    private fun snapToWideCharBoundary(
        gridRow: Int,
        col: Int,
    ): Int {
        if (col <= 0) return col
        val bridge = viewModel?.runtime?.bridge() ?: return col
        // Cache the last queried row (time-bounded): during a handle drag
        // the row is stable for long stretches (only col moves), and
        // scrollbackLine copies a whole line across JNI on every call.
        val nowMs = SystemClock.uptimeMillis()
        // During an active handle drag the cache is session-scoped
        // (spec constraint b): rows only change via edge-scroll which
        // re-latches anyway; otherwise the 500ms TTL applies.
        val line =
            if (
                gridRow == cachedWideCharLineRow &&
                (
                    dragWideCharCacheSession ||
                        nowMs - cachedWideCharLineAtMs < WIDE_CHAR_CACHE_TTL_MS
                    )
            ) {
                cachedWideCharLine
            } else {
                bridge.scrollbackLine(gridRow)?.also {
                    cachedWideCharLineRow = gridRow
                    cachedWideCharLine = it
                    cachedWideCharLineAtMs = nowMs
                }
            } ?: return col
        return snapColToWideChar(line, col)
    }

    private fun latchDragAnchor(
        which: HandleDrag,
        pointerId: Int? = null,
    ) {
        handleDragState = which
        // Lock onto the finger that started the drag: subsequent MOVE events
        // from other pointers must not steer the selection (issue #15).
        dragPointerId = pointerId
        // Round-234 (spec text-selection "drag fluency" constraint b):
        // snapshot per-drag state so MOVE frames issue ZERO JNI calls.
        dragAltScreenSnapshot =
            runCatching { viewModel?.runtime?.bridge()?.isAltScreenActive() ?: false }
                .getOrDefault(false)
        dragWideCharCacheSession = true
        // Round-237: call setSelectionDragging(true) ONCE at drag start so the
        // render thread suppresses new-output scroll reset. Previously this was
        // called per-MOVE inside dragSelection — wasteful since it's idempotent.
        viewModel?.runtime?.setSelectionDragging(true)
        // D7.5 completion ( review): a handle grab on a paste-only
        // selection upgrades it to a text selection so dragging grows the range
        // instead of being swallowed by the paste-only immutability guard.
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

    /** alt-screen flag snapshotted at drag start (MOVE frames must not JNI). */
    private var dragAltScreenSnapshot = false

    /** True while a handle drag session keeps the wide-char line cache alive. */
    private var dragWideCharCacheSession = false
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
    private var edgeScrollRunnable: Runnable = Runnable {}

    val isSelectingText: Boolean
        get() = viewModel?.state?.value?.selection?.active == true

    private val gestureListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent): Boolean {
                // Reset sub-cell accumulator at gesture start so the first
                // onScroll distance is measured from a clean origin.
                scrollAccumulatorPx = 0f
                return true
            }

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
                // Alternate-screen wheel forwarding (Haven research: altScreen
                // wheel consumption). When the remote is on the alternate
                // screen (vim/less/htop), a touch-scroll gesture must be sent
                // to the remote as mouse-wheel escapes rather than scrolling
                // local scrollback — otherwise the user cannot scroll inside
                // those programs. Forward one wheel event per scrolled row,
                // matching the external-mouse path in onGenericMotionEvent.
                val altBridge = viewModel?.runtime?.bridge()
                if (altBridge != null && altBridge.isAltScreenActive()) {
                    val cellW = viewModel?.runtime?.cellWidth ?: 1f
                    val cellH = viewModel?.runtime?.cellHeight ?: 1f
                    val x = e2.x
                    val y = e2.y
                    // One full cell-height of travel = one wheel row, matching
                    // the local-scroll mapping below (distanceY / cellHeight).
                    val lines = kotlin.math.max(1, kotlin.math.abs((distanceY / cellH).toInt()))
                    // finger up (distanceY > 0) = wheel-up (3, older), finger down = wheel-down (4, newer)
                    val button = if (distanceY > 0f) 3 else 4
                    var forwarded = false
                    repeat(lines) {
                        if (altBridge.encodeMouseEvent(x, y, 0, button, cellW, cellH)) {
                            altBridge.encodeMouseEvent(x, y, 1, button, cellW, cellH)
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
                // Sub-cell accumulator: distanceY < cellHeight must not be dropped
                // — otherwise slow drags produce 0 rows and feel卡顿/闪烁. Accumulate
                // and emit whole rows only, carrying remainder to the next onScroll.
                // Direction: finger UP (distanceY>0, previousY - currentY) → older history
                // (offset increases, viewportTop = scrollbackLength - offset decreases) →
                // termux TerminalView:170-187 mTopRow-- parity, verified against termux-app.
                // Use floor() for symmetric slow thresholds: trunc 0.9→0 but -0.9→0 would stall
                // negative drags; floor -0.9→-1 keeps both directions equally responsive.
                scrollAccumulatorPx += distanceY
                val ch = cellHeight.coerceAtLeast(1f)
                val rawAmount = floor((scrollAccumulatorPx / ch).toDouble()).toInt()
                if (rawAmount != 0) {
                    scrollAccumulatorPx -= rawAmount * ch
                    val newOffset = (scrollOffset + rawAmount).coerceIn(0, scrollbackLen)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                    }
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
                // On the alternate screen (vim/less/htop), a fling must not
                // scroll local scrollback — the gesture belongs to the remote.
                // We drop it here (consuming it) rather than forwarding, since
                // fling velocity has no clean wheel-line mapping; drag-scroll
                // (onScroll) already forwards per-row wheel events.
                val flingBridge = viewModel?.runtime?.bridge()
                if (flingBridge != null && flingBridge.isAltScreenActive()) {
                    return true
                }
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
                // (standard gesture coordinates): finger DOWN → newest (offset decreases). Feeding
                // -velocityY
                // into the scroller makes currY move toward larger offsets
                // for downward flings, matching the corrected drag direction.
                stopFlingAnimation()
                isScrolling = true
                onScrollingStateChanged?.invoke(true)
                flingScroller.fling(
                    0,
                    scrollOffset,
                    0,
                    (-velocityY).toInt(),
                    0,
                    0,
                    0,
                    scrollbackLen,
                )
                postOnAnimation(flingStepRunnable)
                return true
            }

            override fun onSingleTapUp(event: MotionEvent): Boolean {
                // Multi-tap selection (ghostty-android pattern): count
                // rapid taps and handle word/line/select-all on tap 2/3/4+.
                val now = SystemClock.uptimeMillis()
                tapCount = nextTapCount(now, lastTapTime, tapCount, DOUBLE_TAP_WINDOW_MS)
                lastTapTime = now

                if (handleMultiTap(event)) return true

                // 300ms hide-protection (termux :57-80): the first tap right
                // after a handle-drag release is part of finishing the drag
                // gesture, not "tap outside the selection → dismiss menu".
                if (shouldSuppressTapAfterDragEnd(now, lastHandleDragEndUptimeMs)) {
                    return true
                }

                if (isAfterLongPress) {
                    isAfterLongPress = false
                    longPressDragging = false
                    return true
                }
                // the drawer close animation lets a scrim tap
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
                    scrollAccumulatorPx = 0f
                    onScrollingStateChanged?.invoke(false)
                    return true
                }
                // OSC 8 hyperlink tap (termux TerminalView.openLinkAt
                // pattern): a tap on a hyperlink cell opens the URI instead
                // of raising the keyboard.
                if (openLinkAt(event.x, event.y)) {
                    return true
                }
                if (isSelectingText) {
                    selectionHandles.hideSelectionHandles()
                    viewModel?.clearSelection()
                    post {
                        // minSdk 33: the platform WindowInsetsController is
                        // available directly; ViewCompat's helper is deprecated.
                        val controller = windowInsetsController
                        controller?.hide(
                            android.view.WindowInsets.Type.ime(),
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
                    val controller = windowInsetsController
                    controller?.show(
                        android.view.WindowInsets.Type.ime(),
                    )
                }
                return true
            }

            override fun onDoubleTap(event: MotionEvent): Boolean {
                // Multi-tap is handled in onSingleTapUp using tapCount self-counting
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
                    zoomBaseFontSizeSp = viewModel?.runtime?.appliedFontSizeSp() ?: return false
                    zoomActive = true
                    // scaleFactor doubles as the long-press guard: while a
                    // pinch owns the touch sequence, onLongPress skips.
                    scaleFactor = 1.0f
                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    if (!zoomActive || isSelectingText) return false
                    scaleFactor *= detector.scaleFactor
                    val sizeSp =
                        (zoomBaseFontSizeSp * scaleFactor).coerceIn(
                            ZOOM_FONT_SIZE_MIN_SP,
                            ZOOM_FONT_SIZE_MAX_SP,
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
                    val sizeSp =
                        (zoomBaseFontSizeSp * scaleFactor).coerceIn(
                            ZOOM_FONT_SIZE_MIN_SP,
                            ZOOM_FONT_SIZE_MAX_SP,
                        )
                    scaleFactor = 1.0f
                    if (kotlin.math.abs(sizeSp - zoomBaseFontSizeSp) > ZOOM_FONT_SIZE_EPSILON_SP) {
                        // The gesture settled on a new size: persist + full
                        // apply (single grid reflow).
                        onZoomChanged?.invoke(sizeSp)
                    } else {
                        // Back to the anchor size: undo the previews that
                        // already pushed native metrics.
                        onZoomPreview?.invoke(zoomBaseFontSizeSp)
                    }
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
        val scrollbackLength = currentScrollbackLength()
        val col = (x / cellWidth).toInt().coerceIn(0, (cols - 1).coerceAtLeast(0))
        val row = (y / cellHeight).toInt().coerceIn(0, (rows - 1).coerceAtLeast(0))
        val gridRow = (scrollbackLength - scrollOffset + row)

        // Always attempt smart word selection on long-press. The isCellEmpty
        // check is unreliable because the GPU render path (CellData) and the
        // query path (grid_ref) use different data sources. If the cell is
        // genuinely empty, SelectionExpander will return a zero-width range
        // and we fall through to the single-cell invert + paste menu.
        val line = bridge?.scrollbackLine(gridRow)
        // Blank target = null row, whitespace cell, OR any column past the
        // end of the line — termux's getSelectedText(x,y,x,y) returns ""
        // for all three, so they must all classify as paste-only. The old
        // `col < line.length` conjunct classified end-of-line columns as
        // TEXT, which is exactly why long-pressing right of the prompt
        // showed the full menu with PASTE ( root cause A1).
        val isOnWhitespace = isWhitespaceCell(line, col)

        if (isOnWhitespace) {
            viewModel?.setSelectionMode(SelectionMode.Word)
            viewModel?.startSelection(gridRow, col, TouchClass.Whitespace)
            viewModel?.endSelection()

            // Round-234 (termux parity, design D7.5): a blank-cell selection
            // also shows both handles stacked on the cell — dragging either
            // one grows a range selection from blank space, exactly like
            // termux's setInitialTextSelectionPosition flow.
            selectionHandles.showSelectionHandles(gridRow, col, gridRow, col, getAccentColor())

            Log.d(
                "Selection",
                "LONG_PRESS whitespace: row=$row col=$col gridRow=$gridRow " +
                    "mode=Word menu=PASTE_ONLY",
            )
        } else {
            // termux-app style word bounds (TerminalEmulator
            // getWordBoundsAtIndex): expand to the whitespace-delimited run
            // containing the tap — no core SelectionExpander divergence, and
            // long-press and double-tap share exactly this logic.
            val bounds = whitespaceWordBounds(bridge, gridRow, col)

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
                viewModel?.setSelectionMode(SelectionMode.Word)
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
                    "mode=Word menu=FULL cellW=$cellWidth cellH=$cellHeight rows=$rows cols=$cols",
            )

            viewModel?.startSelection(startRow, startCol, TouchClass.Text)
            viewModel?.updateSelection(endRow, endCol)
            viewModel?.endSelection()
            selectionHandles.showSelectionHandles(startRow, startCol, endRow, endCol, getAccentColor())
        }
    }

    /**
     * termux-app style word bounds (TerminalEmulator.getWordBoundsAtIndex): expand (gridRow, col)
     * outward to the whitespace-delimited run that contains it. Single-line, like termux. A tap on
     * whitespace or past the end of the line returns null and the caller falls back to the single
     * cell (paste-only target). Long-press and double-tap share this exact expansion so both gestures
     * select identically.
     */
    private fun whitespaceWordBounds(
        bridge: Bridge?,
        gridRow: Int,
        col: Int,
    ): Pair<Pair<Int, Int>, Pair<Int, Int>>? {
        val line = bridge?.scrollbackLine(gridRow) ?: return null
        val length = line.length
        if (col >= length || line.getOrNull(col)?.isWhitespace() == true) return null
        var start = col
        while (start > 0 && !line[start - 1].isWhitespace()) start--
        var end = col
        while (end < length && !line[end].isWhitespace()) end++
        return (gridRow to start) to (gridRow to (end - 1))
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
        installAccessibilityCustomActions()
        edgeScrollRunnable = Runnable {
            if (!edgeScrollRunning) return@Runnable
            when (pendingEdgeScroll) {
                1 -> {
                    val scrollbackLen = currentScrollbackLength()
                    val newOffset = (scrollOffset + 1).coerceAtMost(scrollbackLen)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                        // Top viewport row in grid coordinates.
                        updateDragHandleForCell(scrollbackLen - newOffset)
                    }
                }

                -1 -> {
                    val newOffset = (scrollOffset - 1).coerceAtLeast(0)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                        // Bottom viewport row in grid coordinates.
                        updateDragHandleForCell(currentScrollbackLength() - newOffset + rows - 1)
                    }
                }
            }
            val selection = viewModel?.state?.value?.selection
            if (selection?.start != null && selection?.end != null) {
                selectionHandles.repositionHandle(
                    HandleDrag.START,
                    selection.start.row,
                    selection.start.col,
                )
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
            val controller = windowInsetsController
            controller?.show(
                android.view.WindowInsets.Type.ime(),
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
            // Spec ime-translation: any IME inset delta must dismiss the selection
            // handles + context menu — popups positioned at show time can never be
            // stale relative to the pan. Do NOT resize here; the hybrid
            // pan-then-reflow defers the single grid reflow to onImeSettled(48ms).
            viewModel?.clearSelection()
            selectionHandles.hideSelectionHandles()
            hideSelectionMenu()
        }
        return result
    }

    /**
     * Called once per IME transition after the 48ms settle window (3×16ms) by
     * TerminalScreen's LaunchedEffect. Performs the single settled reflow:
     * `applyGridResize` → `recomputeGrid` → `attachSurface(reconfigure)`.
     * `onApplyWindowInsets` deliberately does NOT resize per frame — it only
     * records `lastImeBottom` and clears selection.
     */
    fun onImeSettled(settledBottom: Int) {
        if (settledBottom == lastImeBottom && lastImeBottom != 0) {
            // Already at settled value but ensure one reflow if grid never caught up
            // (e.g. first show after cold start where cell metrics were 0).
        }
        lastImeBottom = settledBottom
        if (width <= 0 || height <= 0) return
        resizeManager.applyGridResize(width, height, settledBottom)
    }

    /**
     * Compute the grid from the window size and the current IME inset, then align the PTY. Single
     * formula shared by the insets path and applySurfaceResize so both can never diverge: rows
     * exclude the IME inset and the ModifierBar overlay.
     */
    fun initialize(viewModel: TerminalViewModel) {
        this.viewModel = viewModel
        // wire the render-loop frame hook so the accessibility
        // description tracks terminal output (the SurfaceView is drawn by
        // native code — no other content-changed signal exists).
        viewModel.runtime.onFrameRendered = { accessibilityRenderTick() }
        val handler = terminal.emulator.shortcut.KeyShortcutHandler(viewModel)
        handler.setBindings(viewModel.shortcutBindings.value)
        this.shortcutHandler = handler
        // Reactive subscription: re-push bindings when the user edits shortcuts
        // in Settings  fix for stale handler snapshot).
        surfaceScope?.cancel()
        val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())
        this.surfaceScope = scope
        scope.launch {
            viewModel.shortcutBindings.collect { bindings ->
                shortcutHandler?.setBindings(bindings)
            }
        }
    }

    /**
     * called from the render loop (render thread) after each presented frame. Refreshes the
     * accessibility contentDescription so TalkBack reads live terminal output. The blocking JNI
     * scrollback query runs here (never on the main thread) and is throttled; the description
     * assembly happens on the main thread.
     */
    fun accessibilityRenderTick() {
        if (!isAccessibilityEnabled()) return
        if (accessibilityDescriptionRefreshPosted) return
        val now = System.nanoTime()
        if (
            now - lastAccessibilityScrollbackQueryNanos < ACCESSIBILITY_SCROLLBACK_QUERY_INTERVAL_NANOS
        ) {
            return
        }
        val bridge = viewModel?.runtime?.bridge() ?: return
        lastAccessibilityScrollbackQueryNanos = now
        val scrollbackLength =
            try {
                bridge.scrollbackLength()
            } catch (exception: Exception) {
                LogUtil.w(TAG, "accessibility scrollbackLength query failed", exception)
                return
            }
        accessibilityScrollbackLength = scrollbackLength
        // Per-line scrollback queries are blocking JNI: each one locks the
        // session, which the render thread holds for the whole frame
        // (~500 ms under software rendering). Running them on the main
        // thread would pin the main looper inside a mutex for every frame,
        // which keeps Espresso/Compose idling permanently busy. Assemble
        // the description HERE on the render thread; the main thread only
        // applies the resulting string (no JNI).
        val lines = accessibilityLineProvider.visibleLines(rows, scrollbackLength, scrollOffset)
        val description = accessibilityLineProvider.contentDescription(lines)
        if (description.isEmpty()) return
        accessibilityDescriptionRefreshPosted = true
        accessibilityMainHandler.post {
            accessibilityDescriptionRefreshPosted = false
            if (isAccessibilityEnabled()) {
                accessibilityDescriptionUpdater.update(description) { this.contentDescription = it }
            }
        }
    }

    /**
     * Next (delta > 0) / Previous (delta < 0) / current (delta == 0) accessibility line navigation.
     * The chosen line becomes the contentDescription immediately (no debounce) so TalkBack reads it
     * right after the custom action completes. Main thread only.
     */
    private fun navigateAccessibilityLine(delta: Int) {
        val bridge = viewModel?.runtime?.bridge() ?: return
        if (!isAccessibilityEnabled()) return
        val scrollbackLength = currentScrollbackLength()
        val line =
            when {
                delta > 0 -> accessibilityNavigator.next(rows, scrollbackLength, scrollOffset)
                delta < 0 -> accessibilityNavigator.previous(rows, scrollbackLength, scrollOffset)
                else -> accessibilityNavigator.current(rows, scrollbackLength, scrollOffset)
            }
        if (line != null) {
            accessibilityDescriptionUpdater.cancel()
            contentDescription = line.text
        }
    }

    private fun isAccessibilityEnabled(): Boolean {
        val manager =
            context.getSystemService(android.content.Context.ACCESSIBILITY_SERVICE)
                as? android.view.accessibility.AccessibilityManager ?: return false
        return manager.isEnabled
    }

    private fun installAccessibilityCustomActions() {
        // API 37 removed the ACTION_CUSTOM_ACTION constant; custom action
        // ids still start at 0x1000 (see AccessibilityNodeInfo docs).
        val customBase = ACCESSIBILITY_CUSTOM_ACTION_BASE
        accessibilityDelegate =
            object : android.view.View.AccessibilityDelegate() {
                override fun onInitializeAccessibilityNodeInfo(
                    host: android.view.View,
                    info: android.view.accessibility.AccessibilityNodeInfo,
                ) {
                    super.onInitializeAccessibilityNodeInfo(host, info)
                    info.addAction(
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                            customBase + 1,
                            context.getString(R.string.accessibility_next_line),
                        ),
                    )
                    info.addAction(
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                            customBase + 2,
                            context.getString(R.string.accessibility_previous_line),
                        ),
                    )
                    info.addAction(
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                            customBase + 3,
                            context.getString(R.string.accessibility_read_screen),
                        ),
                    )
                }

                // API 37 renamed View.AccessibilityDelegate.onPerformAccessibilityAction
                // to performAccessibilityAction (Android 16 accessibility overhaul).
                override fun performAccessibilityAction(
                    host: android.view.View,
                    action: Int,
                    arguments: android.os.Bundle?,
                ): Boolean = when (action) {
                    customBase + 1 -> {
                        navigateAccessibilityLine(1)
                        true
                    }

                    customBase + 2 -> {
                        navigateAccessibilityLine(-1)
                        true
                    }

                    customBase + 3 -> {
                        navigateAccessibilityLine(0)
                        true
                    }

                    else -> super.performAccessibilityAction(host, action, arguments)
                }
            }
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

    /**
     * Absolute grid row displayed at the viewport top (scrollbackLength - scrollOffset): the single
     * source for the old inline `scrollbackLength - scrollOffset` formula that selection rectangles
     * and drag/cursor handles all need.
     */
    private fun currentViewportTopGrid(): Int = currentScrollbackLength() - scrollOffset

    fun scrollToRow(row: Int) {
        stopFlingAnimation()
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

    /**
     * Reset the local scroll offset to the session's offset ): called on session switch so selection
     * coordinate math does not use the previous session's offset.
     */
    fun resetScrollOffset() {
        stopFlingAnimation()
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

    /**
     * Handle multi-tap selection (ghostty-android pattern). Returns true if the event was consumed
     * (tapCount >= 2).
     */
    private fun handleMultiTap(event: MotionEvent): Boolean {
        when (multiTapAction(tapCount)) {
            MultiTapAction.SELECT_ALL -> {
                viewModel?.selectAll()
                showHandlesIfActive()
                return true
            }

            MultiTapAction.LINE -> {
                startSelectionAt(event, selectLine = true)
                showHandlesIfActive()
                return true
            }

            MultiTapAction.WORD -> {
                startSelectionAt(event, expandToWord = true)
                showHandlesIfActive()
                return true
            }

            MultiTapAction.NOT_A_MULTI_TAP -> return false
        }
        return false
    }

    private fun showHandlesIfActive() {
        val sel = viewModel?.state?.value?.selection ?: return
        if (sel.active && sel.start != null && sel.end != null) {
            selectionHandles.showSelectionHandles(
                sel.start.row,
                sel.start.col,
                sel.end.row,
                sel.end.col,
                getAccentColor(),
            )
        }
    }

    private fun startSelectionAt(
        event: MotionEvent,
        expandToWord: Boolean = false,
        selectLine: Boolean = false,
    ) {
        val col = pixelToCell(event.x, cellWidth, cols)
        val row = pixelToCell(event.y, cellHeight, rows)

        if (selectLine) {
            // Triple-tap line selection: select the entire line at the tap row.
            val scrollbackLength = currentScrollbackLength()
            val gridRow = scrollbackLength - scrollOffset + row
            viewModel?.setSelectionMode(SelectionMode.Line)
            viewModel?.startSelection(gridRow, 0)
            val bridge = viewModel?.runtime?.bridge()
            val line = bridge?.scrollbackLine(gridRow) ?: ""
            viewModel?.updateSelection(gridRow, line.length.coerceAtLeast(0))
            viewModel?.endSelection()
            Log.d(
                "Selection",
                "TRIPLE_TAP line: tapRow=$row gridRow=$gridRow lineLen=${line.length}",
            )
        } else if (expandToWord) {
            // termux-app whitespace word bounds — identical expansion to
            // long-press (see handleLongPress), no core-backed divergence.
            val bridge = viewModel?.runtime?.bridge()
            val scrollbackLength = currentScrollbackLength()
            val gridRow = scrollbackLength - scrollOffset + row
            val bounds = whitespaceWordBounds(bridge, gridRow, col)
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
                viewModel?.startSelection(scrollbackLength - scrollOffset + row, col)
            }
        } else {
            val scrollbackLength = currentScrollbackLength()
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
        return KeyModifiers.fromKeyEvent(
            event,
            state?.ctrlState ?: ModifierState.Off,
            state?.altState ?: ModifierState.Off,
        )
    }

    override fun onKeyDown(
        keyCode: Int,
        event: KeyEvent,
    ): Boolean {
        // Hardware shortcut intercept: ask the handler first.  Only bound
        // chords (Ctrl+Shift+V, etc.) are consumed; unbound keys (Ctrl+C
        // SIGINT, Ctrl+D EOF, …) always fall through to the terminal.
        shortcutHandler?.let { handler ->
            if (handler.dispatch(event)) return true
        }
        // while a selection is active, arrow keys move the
        // selection START anchor (termlib moveSelection* semantics) instead
        // of emitting escape sequences to the shell. Hardware arrows only —
        // physical keyboards set keyCode; soft IME keys come through as text.
        val activeSelection = viewModel?.state?.value?.selection
        if (
            activeSelection?.active == true &&
            activeSelection.start != null &&
            activeSelection.end != null
        ) {
            val delta =
                when (keyCode) {
                    KeyEvent.KEYCODE_DPAD_LEFT -> 0 to -1
                    KeyEvent.KEYCODE_DPAD_RIGHT -> 0 to 1
                    KeyEvent.KEYCODE_DPAD_UP -> -1 to 0
                    KeyEvent.KEYCODE_DPAD_DOWN -> 1 to 0
                    else -> null
                }
            if (delta != null) {
                viewModel?.moveSelectionAnchorBy(delta.first, delta.second)
                return true
            }
        }
        val terminalViewModel = viewModel
        // Round-234 fix 2026-08-23: hardware Enter (including maestro
        // pressKey and adb keyevent) bypasses writeToPty via
        // bridge.processKeyEvent — the reported "new command + Enter does
        // not scroll" root cause. Share the input-driven snap so ANY Enter
        // snaps immediately, and any hardware input clears the SCROLL lock.
        if (terminalViewModel != null) {
            val isEnter =
                keyCode == KeyEvent.KEYCODE_ENTER ||
                    keyCode == KeyEvent.KEYCODE_NUMPAD_ENTER ||
                    keyCode == KeyEvent.KEYCODE_DPAD_CENTER
            terminalViewModel.onUserInputForScrollSnap(isEnter)
        }
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
        // Key-up is intentionally NOT forwarded to the bridge: Bridge's
        // processKeyEvent drops every non-ACTION_DOWN (writing on UP would
        // double each keystroke), so it always returns false here. Fall
        // through to the system so key-up semantics (long-press repeat,
        // system gestures) are not swallowed.
        return super.onKeyUp(keyCode, event)
    }

    private fun handleKeyEvent(event: KeyEvent): Boolean = onKeyDown(event.keyCode, event)

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4: Touch event dispatch
    // ══════════════════════════════════════════════════════════════════════

    @Suppress(
        "CyclomaticComplexMethod",
        "LongMethod",
        "NestedBlockDepth",
    ) // Acceptable — dispatches ~15 distinct gesture/intent types
    override fun performClick(): Boolean = super.performClick()

    // Acceptable: dispatches ~15 distinct gesture/intent types with
    // selection, scroll, long-press, and hardware-key interactions.
    @Suppress("LongMethod", "CyclomaticComplexMethod", "CognitiveComplexMethod")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_UP) {
            // Accessibility contract: a view overriding onTouchEvent must
            // call performClick on touch-up so TalkBack click actions work.
            performClick()
        }
        if (!touchEnabled) {
            scaleDetector.onTouchEvent(event)
            gestureDetector.onTouchEvent(event)
            return false
        }
        if (event.action == MotionEvent.ACTION_DOWN) {
            parent?.requestDisallowInterceptTouchEvent(true)
            // A new touch stops any in-flight fling animation (standard
            // Android scrollable behavior; termux does the same in onTouchEvent).
            stopFlingAnimation()
        }
        // the drawer's swipe-from-edge gesture starts in the
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
            // Mouse-mode reporting (DECSET 1000/1002/1003): route mouse
            // events to the terminal via the Ghostty mouse encoder (zelland
            // src-tauri/src/terminal.rs encode_mouse_event pattern). The
            // encoder returns an empty sequence when the application has not
            // enabled mouse reporting, in which case the event falls through
            // to the app gestures below (right-click word-select,
            // middle-click paste).
            val runtime = viewModel?.runtime
            val bridge = runtime?.bridge()
            if (bridge != null) {
                val cellW = runtime.cellWidth
                val cellH = runtime.cellHeight
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
                if (bridge.encodeMouseEvent(event.x, event.y, action, button, cellW, cellH)) {
                    return true
                }
            }

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
                    // Round-234: handle drags are owned by the single
                    // overlay window (spec "生命周期单一 owner") — a DOWN
                    // on a handle never reaches the surface anymore. Any
                    // DOWN that DOES arrive here is a non-handle tap:
                    // clear the selection exactly as before.
                    viewModel?.clearSelection()
                    selectionHandles.hideSelectionHandles()
                    hideSelectionMenu()
                }
            }

            MotionEvent.ACTION_MOVE -> {
                if (isSelectingText && handleDragState != HandleDrag.NONE) {
                    // Pointer-id lock (issue #15): only the finger that
                    // latched the drag may steer the selection. Coordinates
                    // come from that pointer's slot; moves carried by a second
                    // finger — or after it lifted — are swallowed.
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
                    val gridRow = currentScrollbackLength() - scrollOffset + row
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

            MotionEvent.ACTION_UP,
            MotionEvent.ACTION_CANCEL,
            -> {
                if (longPressDragging) {
                    longPressDragging = false
                    // Long-press drag also ends a selection gesture: arm the
                    // tap guard so the release tap cannot dismiss the menu.
                    lastHandleDragEndUptimeMs = SystemClock.uptimeMillis()
                    val sel = viewModel?.state?.value?.selection
                    if (sel?.start != null && sel?.end != null) {
                        viewModel?.endSelection()
                        // Round-234 (termux parity, design D7.5): paste-only
                        // single-cell selections ALSO get handles — dragging
                        // them is how a range selection starts from blank
                        // space. The toolbar anchors above the handles via
                        // onGetContentRect's handle-height offset.
                        selectionHandles.showSelectionHandles(
                            sel.start.row,
                            sel.start.col,
                            sel.end.row,
                            sel.end.col,
                            getAccentColor(),
                        )
                    }
                }
                edgeScrollRunning = false
                pendingEdgeScroll = 0
                edgeScrollHandler.removeCallbacks(edgeScrollRunnable)
                if (isSelectingText && handleDragState != HandleDrag.NONE) {
                    finishHandleDrag()
                }
                handleDragState = HandleDrag.NONE
                dragPointerId = null
                dragWideCharCacheSession = false
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

    /**
     * Mouse wheel events (external mouse / trackpad). When the application has mouse reporting
     * enabled (DECSET 1000/1002/1003), wheel events are encoded as buttons 4/5 by the Ghostty mouse
     * encoder and written to the PTY (zelland src-tauri/src/terminal.rs: scroll_up/scroll_down →
     * button FOUR/FIVE). The encoder returns an empty sequence when reporting is off — the event is
     * then ignored (there is no scrollback wheel handling; touch scrolling covers that).
     */
    override fun onGenericMotionEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_SCROLL && event.isFromSource(InputDevice.SOURCE_MOUSE)) {
            val runtime = viewModel?.runtime
            val bridge = runtime?.bridge()
            if (bridge != null) {
                val delta = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
                val button = if (delta > 0f) 3 else 4 // wheel-up=3, wheel-down=4 (Rust mapping)
                if (
                    bridge.encodeMouseEvent(
                        event.x,
                        event.y,
                        0,
                        button,
                        runtime.cellWidth,
                        runtime.cellHeight,
                    )
                ) {
                    // Wheel release completes the scroll gesture.
                    bridge.encodeMouseEvent(
                        event.x,
                        event.y,
                        1,
                        button,
                        runtime.cellWidth,
                        runtime.cellHeight,
                    )
                    return true
                }
            }
        }
        return super.onGenericMotionEvent(event)
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
     * Reconfigure the native (wgpu) surface + grid to [width]x[height] right now. Idempotent: a no-op
     * when the size already matches the last configured size. Must run on the main thread (holds the
     * bridge surface lock while the render thread may briefly contend, but never deadlocks).
     */

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 5: Surface lifecycle (ResizeManager owns grid/size)
    // ══════════════════════════════════════════════════════════════════════

    override fun surfaceCreated(holder: SurfaceHolder) {
        // 0-size guard: layout may call surfaceCreated before the view is measured
        // (width/height 0) — binding a 0×0 ANativeWindow creates a black swapchain
        // that acquireNextImage fails on (flash). Defer until surfaceChanged with real size.
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
                // ADR-0007: re-attach the (recreated) surface; the renderer
                // rebuilds its wgpu surface from it.
                terminalViewModel.runtime.attachSurface(surface, width, height)
                terminalViewModel.runtime.recomputeGrid(width, height)
                // onSurfaceDestroyed set render_paused=true on this path;
                // it is only cleared by the settings screen, so a plain
                // background/resume cycle left the flag set and the
                // restarted render thread produced black frames
                // (render_frame short-circuits when paused). Clear it
                // before the thread restarts.
                terminalViewModel.runtime.setRenderPaused(false)
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
        if (width <= 0 || height <= 0) return
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
        stopEdgeScroll()
        // Reset any latch left by a drag interrupted by surface teardown
        // (review-6 hygiene): the overlay is gone so no UP will arrive;
        // a stale latch would be self-healed only by the next touch.
        handleDragState = HandleDrag.NONE
        dragPointerId = null
        dragWideCharCacheSession = false
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

// ─────────────────────────────────────────────────────────────────────────────
// Pure helpers (top-level, unit-testable without a view / bridge)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Next multi-tap count: rapid tap within [windowMs] increments, older resets to 1 (strict `<` — a
 * tap exactly at the window edge starts a fresh click). Backs onSingleTapUp's tap counter.
 */
internal fun nextTapCount(now: Long, lastTapTime: Long, tapCount: Int, windowMs: Long): Int = if (now - lastTapTime < windowMs) tapCount + 1 else 1

/**
 * Selection action for a tap count (ghostty-android pattern): 2 taps → word, 3 → line, 4+ →
 * select-all; 1 is not consumed. Backs handleMultiTap.
 */
internal enum class MultiTapAction {
    NOT_A_MULTI_TAP,
    WORD,
    LINE,
    SELECT_ALL,
}

internal fun multiTapAction(tapCount: Int): MultiTapAction = when {
    tapCount >= 4 -> MultiTapAction.SELECT_ALL
    tapCount == 3 -> MultiTapAction.LINE
    tapCount == 2 -> MultiTapAction.WORD
    else -> MultiTapAction.NOT_A_MULTI_TAP
}

/**
 * Edge-scroll zone for a drag y: top half-cell → scroll up, bottom half-cell → scroll down, middle
 * → stop. Matches the asymmetrical boundaries in the drag handler (`<` top vs `>=` bottom) so a
 * degenerate surface (height < cellHeight) favors the top zone.
 */
internal enum class EdgeScrollDirection {
    UP,
    DOWN,
    STOP,
}

internal fun edgeScrollDirection(
    y: Float,
    surfaceHeightPx: Float,
    cellHeight: Float,
): EdgeScrollDirection = when {
    y < cellHeight / 2 -> EdgeScrollDirection.UP
    y >= surfaceHeightPx - cellHeight / 2 -> EdgeScrollDirection.DOWN
    else -> EdgeScrollDirection.STOP
}

/**
 * Pixel offset → clamped grid cell (0..maxCells-1; maxCells 0 stays 0). Backs openLinkAt and the
 * drag target mapping.
 */
internal fun pixelToCell(px: Float, cellSize: Float, maxCells: Int): Int = (px / cellSize).toInt().coerceIn(0, (maxCells - 1).coerceAtLeast(0))

/**
 * Snap a selection column left of a wide character's trailing half (the pure core of
 * snapToWideCharBoundary given a fetched line; the bridge/row-cache part stays in the surface).
 */
internal fun snapColToWideChar(line: String, col: Int): Int {
    if (col <= 0) return col
    var cell = 0
    var i = 0
    while (i < line.length) {
        val cp = line.codePointAt(i)
        val width = if (isWideCodePoint(cp)) 2 else 1
        if (cell + width > col) {
            // col inside this char: snap back only on a wide char's
            // trailing half — the leading half and ASCII stay in place.
            if (width == 2 && col == cell + 1) return col - 1
            return col
        }
        cell += width
        i += Character.charCount(cp)
    }
    return col
}

/**
 * Map an absolute scrollback-grid coordinate (row, col) to viewport pixel coordinates.
 * `viewportTopGrid` is the absolute grid row shown at the top of the viewport (scrollbackLength -
 * scrollOffset); pass 0 for a row that is already viewport-relative. Extracted from the inline
 * `row - (scrollbackLength - scrollOffset)` formulas so scrolling, search-jump and font-size
 * changes share one testable conversion.
 *
 * Returns un-rounded pixels; callers round and clamp to view bounds themselves (handles anchor at
 * row bottoms by passing row + 1).
 */
internal fun gridToScreen(
    row: Int,
    col: Int,
    viewportTopGrid: Int,
    cellWidth: Float,
    cellHeight: Float,
): Pair<Float, Float> = Pair(col * cellWidth, (row - viewportTopGrid) * cellHeight)

/**
 * Normalized selection bounds: start ≤ end, both anchors clamped to the grid. Produced by
 * [clampSelection]; consumed by the drag-handle update path so native setSelection never sees
 * inverted or out-of-bounds cells.
 */
internal data class SelectionBounds(
    val startRow: Int,
    val startCol: Int,
    val endRow: Int,
    val endCol: Int,
)

/**
 * Order-preserving range clamp for a selection (termux TextSelectionCursorController.updatePosition
 * semantics):
 *
 * 1. clamp each anchor into `[0,maxRow] × [0,maxCol]` (negative max → empty grid → all zeros);
 * 2. swap the anchors when they came in inverted so the returned bounds always satisfy start ≤ end.
 *
 * Pure; backs the drag-path defense-in-depth in SelectionManager.dragSelection.
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
        // Inverted input: swap so start ≤ end after clamping.
        SelectionBounds(startRow = er, startCol = ec, endRow = sr, endCol = sc)
    } else {
        SelectionBounds(startRow = sr, startCol = sc, endRow = er, endCol = ec)
    }
}

/**
 * True when the long-press target cell is a paste-only (blank) target: a null row, a whitespace
 * cell, OR any column past the end of the line. termux's getSelectedText(x,y,x,y) returns "" for
 * all three, so they all classify as blank — the old `col < line.length` conjunct classified
 * end-of-line columns as TEXT and surfaced the full menu with PASTE there ( root cause A1). Pure;
 * backs handleLongPress.
 */
internal fun isWhitespaceCell(
    line: String?,
    col: Int,
): Boolean = when {
    line == null -> true
    col >= line.length -> true
    else -> line[col].isWhitespace()
}

/**
 * Pointer-id lock for handle drags (issue #15 multi-pointer drift): once a drag is latched by
 * [ownerPointerId], later move events may steer the selection only when they carry that same
 * pointer. A null owner means no drag was latched through the locking path — accept, preserving
 * legacy behavior. A null candidate means the event carries no usable pointer id — reject, a second
 * finger must never hijack an existing drag. Pure; backs the ACTION_MOVE guards in TerminalSurface
 * and the handle popups.
 */
internal fun acceptsDragPointer(
    ownerPointerId: Int?,
    candidatePointerId: Int?,
): Boolean = when {
    ownerPointerId == null -> true
    candidatePointerId == null -> false
    else -> ownerPointerId == candidatePointerId
}

/**
 * Guard window after a handle-drag ends during which a tap at the release position is swallowed
 * instead of dismissing the freshly re-shown menu (termux hide-protection :57-80).
 */
internal const val SELECTION_MENU_RESHOW_GUARD_MS = 300L

/**
 * True while [nowMs] is inside [SELECTION_MENU_RESHOW_GUARD_MS] of the last drag end
 * ([lastDragEndMs], uptimeMillis). The first tap after a drag is consumed as part of finishing the
 * gesture, not as a "tap outside the selection → dismiss" command. Pure; backs onSingleTapUp.
 */
internal fun shouldSuppressTapAfterDragEnd(
    nowMs: Long,
    lastDragEndMs: Long,
    windowMs: Long = SELECTION_MENU_RESHOW_GUARD_MS,
): Boolean = lastDragEndMs > 0L && nowMs >= lastDragEndMs && nowMs - lastDragEndMs < windowMs
