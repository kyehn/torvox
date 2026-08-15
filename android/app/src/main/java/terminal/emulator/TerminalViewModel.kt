package terminal.emulator

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.Surface
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import coil3.BitmapImage
import coil3.ImageLoader
import coil3.request.ImageRequest
import coil3.request.allowHardware
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.bridge.FontInfoDto
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.SelectionExpander
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.KeyboardMode
import terminal.emulator.input.ModifierState
import terminal.emulator.input.next
import terminal.emulator.input.toKeyboardMode
import terminal.emulator.input.toSettingsString
import terminal.emulator.runtime.ClipboardAccess
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.PasteChunker
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.SmartCopy
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.ui.theme.UserThemeStore
import terminal.emulator.ui.theme.resolveTerminalThemeName
import javax.inject.Inject
import androidx.core.net.toUri

private const val CLIPBOARD_TEXT_MAX_LENGTH = 100_000

enum class SelectionMode {
    Char,
    Word,
    Line,
    Block,
    Semantic,
}

/** Mirror of selection::TouchClass. */
enum class TouchClass {
    Text,
    Whitespace,
    EmptyArea,
    Unknown,
}

data class SelectionAnchor(
    val row: Int,
    val col: Int,
)

data class SelectionState(
    val active: Boolean = false,
    val dragging: Boolean = false,
    val start: SelectionAnchor? = null,
    val end: SelectionAnchor? = null,
    val mode: SelectionMode = SelectionMode.Char,
    val selectedText: String = "",
    val touchClass: TouchClass = TouchClass.Unknown,
    // Set when a menu action (Copy/Select All/Share) was taken: the
    // floating menu closes while the selection highlight stays.
    // Reset on the next long-press or drag-handle move.
    val menuDismissed: Boolean = false,
) {
    val pasteOnly: Boolean
        get() = touchClass == TouchClass.EmptyArea || touchClass == TouchClass.Whitespace

    val hasSelection: Boolean
        get() = active && start != null && end != null

    fun applyHandleDrag(
        draggingStart: Boolean,
        targetRow: Int,
        targetCol: Int,
    ): HandleDragResult {
        val currentEnd = end ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol)
        val currentStart = start ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol)
        if (draggingStart && (targetRow > currentEnd.row || (targetRow == currentEnd.row && targetCol >= currentEnd.col))) {
            return HandleDragResult(currentEnd.row, currentEnd.col, targetRow, targetCol)
        }
        if (!draggingStart && (targetRow < currentStart.row || (targetRow == currentStart.row && targetCol <= currentStart.col))) {
            return HandleDragResult(targetRow, targetCol, currentStart.row, currentStart.col)
        }
        if (draggingStart) {
            return HandleDragResult(targetRow, targetCol, currentEnd.row, currentEnd.col)
        }
        return HandleDragResult(currentStart.row, currentStart.col, targetRow, targetCol)
    }

    /**
     * arrow-key selection navigation, termlib moveSelection*
     * semantics): move the START anchor by [deltaRow]/[deltaCol], clamped to
     * the grid so it never crosses the END anchor (the end stays put as the
     * moving end sweeps up to it). Pure and unit-testable.
     */
    fun moveSelectionAnchorBy(
        deltaRow: Int,
        deltaCol: Int,
        maxRow: Int,
        maxCol: Int,
    ): SelectionState {
        val currentStart = start ?: return this
        val currentEnd = end ?: return this
        if (!active) return this
        val newRow = (currentStart.row + deltaRow).coerceIn(0, maxRow.coerceAtLeast(0))
        val newCol = (currentStart.col + deltaCol).coerceIn(0, maxCol.coerceAtLeast(0))
        val crossedEnd =
            newRow > currentEnd.row ||
                (newRow == currentEnd.row && newCol > currentEnd.col)
        val anchor =
            if (crossedEnd) {
                // Clamp just before the end anchor so the range never
                // inverts: same row → col just before END; below END → last
                // row before it with col just before END's col.
                currentStart.copy(
                    row = (currentEnd.row - 1).coerceAtLeast(0),
                    col = (currentEnd.col - 1).coerceAtLeast(0),
                )
            } else {
                currentStart.copy(row = newRow, col = newCol)
            }
        return copy(start = anchor)
    }
}

data class HandleDragResult(
    val startRow: Int,
    val startCol: Int,
    val endRow: Int,
    val endCol: Int,
)

/** Session info for the session drawer. */
data class SessionInfo(
    val id: Long,
    val title: String,
)

data class TerminalState(
    val sessionId: Long = 0L,
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val selection: SelectionState = SelectionState(),
    val ctrlState: ModifierState = ModifierState.Off,
    val altState: ModifierState = ModifierState.Off,
    val scrollActive: Boolean = false,
    val sessions: List<SessionInfo> = emptyList(),
    val activeSessionId: Long = 0L,
    val keyboardMode: KeyboardMode = KeyboardMode.Secure,
    val selectionBg: Int = 0,
    val selectionAccent: Int = 0,
)

internal fun shouldCreateDefaultSession(
    surfaceValid: Boolean,
    surfaceWidth: Int,
    surfaceHeight: Int,
    uiSessions: List<SessionInfo>,
    runtimeSessionIds: List<Long>,
): Boolean = surfaceValid &&
    surfaceWidth > 0 &&
    surfaceHeight > 0 &&
    uiSessions.isEmpty() &&
    runtimeSessionIds.isEmpty()

/**
 * Copies a `content://` URI into `dst` (app-private storage) so the
 * wallpaper never depends on a revocable SAF grant. Returns false when
 * the stream cannot be opened or the copy fails — the caller keeps the
 * original path in that case, and any partially-written `dst` is
 * removed so a failed copy never leaves a corrupt wallpaper file.
 */
internal fun copyContentUriToPrivateFile(
    contentResolver: android.content.ContentResolver,
    uri: android.net.Uri,
    dst: java.io.File,
): Boolean {
    fun copyStream(input: java.io.InputStream): Boolean = java.io.FileOutputStream(dst).use { output ->
        val buf = ByteArray(COPY_BUFFER_SIZE)
        while (true) {
            val n = input.read(buf)
            if (n <= 0) break
            output.write(buf, 0, n)
        }
        true
    }
    return try {
        contentResolver.openInputStream(uri)?.let(::copyStream) ?: false
    } catch (_: Exception) {
        dst.delete()
        false
    }
}

private const val COPY_BUFFER_SIZE = 64 * 1024

// ═══════════════════════════════════════════════════════════════════════════
// SECTION 1: Fields & constructor
// ═══════════════════════════════════════════════════════════════════════════

@HiltViewModel
class TerminalViewModel
@Inject
constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepository: SettingsRepository,
    val runtime: TerminalRuntime,
) : ViewModel() {
    private val clipboardAccess = ClipboardAccess(context, tag = "ViewModel")

    private val selectionManager = SelectionManager()

    // last grid size seen from the runtime; a shrink clamps
    // the active selection (see runtime.state collector).
    @Volatile private var lastGridRows = 0

    @Volatile private var lastGridCols = 0
    private val fontManager = FontManager()
    private val userThemeStore = UserThemeStore(context)

    // Hot StateFlow mirror of the DataStore (cold flow + recomposition would
    // resubscribe per frame and miss updates; stateIn keeps one collector).
    private val _userThemes = kotlinx.coroutines.flow.MutableStateFlow<List<terminal.emulator.ui.theme.TerminalTheme>>(emptyList())
    val userThemes: kotlinx.coroutines.flow.StateFlow<List<terminal.emulator.ui.theme.TerminalTheme>> = _userThemes.asStateFlow()

    // ── Keyboard shortcuts ────────────────────────────────────────────
    private val _shortcutBindings = kotlinx.coroutines.flow.MutableStateFlow(
        terminal.emulator.shortcut.KeyShortcutHandler.Defaults.all(),
    )
    val shortcutBindings: kotlinx.coroutines.flow.StateFlow<Map<String, terminal.emulator.shortcut.ShortcutBinding>> =
        _shortcutBindings.asStateFlow()

    /** Load persisted shortcut bindings from DataStore into the hot StateFlow. */
    private fun loadShortcutBindingsFromDataStore(state: terminal.emulator.settings.SettingsRepository.SettingsState) {
        val defaults = terminal.emulator.shortcut.KeyShortcutHandler.Defaults.all()
        val loaded = defaults.mapValues { (actionId, default) ->
            val serialized = when (actionId) {
                terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_PASTE -> state.shortcutPaste
                terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_NEW_SESSION -> state.shortcutNewSession
                terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_CLOSE_SESSION -> state.shortcutCloseSession
                terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_COPY -> state.shortcutCopy
                terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_TOGGLE_SCROLL -> state.shortcutToggleScroll
                else -> ""
            }
            if (serialized.isNotEmpty()) {
                terminal.emulator.shortcut.ShortcutBinding.deserialize(serialized).let {
                    if (it.isEmpty()) default else it
                }
            } else {
                default
            }
        }
        _shortcutBindings.value = loaded
    }

    fun updateShortcutBinding(actionId: String, binding: terminal.emulator.shortcut.ShortcutBinding) {
        _shortcutBindings.value = _shortcutBindings.value.toMutableMap().apply { put(actionId, binding) }
        viewModelScope.launch {
            settingsRepository.setShortcutBinding(actionId, binding.serialize())
        }
    }

    fun resetShortcutBinding(actionId: String) {
        val defaults = terminal.emulator.shortcut.KeyShortcutHandler.Defaults.all()
        _shortcutBindings.value = _shortcutBindings.value.toMutableMap().apply {
            put(actionId, defaults[actionId] ?: terminal.emulator.shortcut.ShortcutBinding.EMPTY)
        }
        viewModelScope.launch {
            settingsRepository.clearShortcutBinding(actionId)
        }
    }

    fun hasShortcutConflict(actionId: String, binding: terminal.emulator.shortcut.ShortcutBinding): Boolean = _shortcutBindings.value.any { (id, existing) ->
        id != actionId && existing == binding
    }

    // ── Font forwards (implementation in FontManager) ─────────────────────

    fun setFontSize(size: Float) = fontManager.setFontSize(size)

    fun setFontFamily(family: String) = fontManager.setFontFamily(family)

    /** Slot: [FONT_SLOT_BOLD] or [FONT_SLOT_ITALIC] (ghostty-android 4-slot). */
    fun setFontFamilyForStyle(family: String, slot: Int) = fontManager.setFontFamilyForStyle(family, slot)

    fun installFontFile(uri: Uri) = fontManager.installFontFile(uri)

    // ── Selection forwards (implementation in SelectionManager) ────────────

    fun startSelection(
        row: Int,
        col: Int,
        touchClass: TouchClass = TouchClass.Unknown,
    ) = selectionManager.startSelection(row, col, touchClass)

    fun updateSelection(row: Int, col: Int) = selectionManager.updateSelection(row, col)

    fun updateSelectionStart(row: Int, col: Int) = selectionManager.updateSelectionStart(row, col)

    fun endSelection() = selectionManager.endSelection()

    fun setSelectionMode(mode: SelectionMode) = selectionManager.setSelectionMode(mode)

    fun copySelectionToClipboard() = selectionManager.copySelectionToClipboard()

    fun clearSelection() = selectionManager.clearSelection()

    fun showPastePopup(row: Int, col: Int) = selectionManager.showPastePopup(row, col)

    fun shareSelection() = selectionManager.shareSelection()

    fun selectAll(scrollOffset: Int = 0) = selectionManager.selectAll(scrollOffset)

    fun moveSelectionAnchor(deltaCol: Int) = selectionManager.moveSelectionAnchor(deltaCol)

    fun moveSelectionAnchorBy(deltaRow: Int, deltaCol: Int) = selectionManager.moveSelectionAnchorBy(deltaRow, deltaCol)

    fun pasteFromClipboard(): Int = selectionManager.pasteFromClipboard()

    /**
     * clamp a selection's anchors onto [rows]×[cols] after a
     * grid resize so native setSelection never receives out-of-bounds cells.
     */
    private fun clampSelectionToGrid(selection: SelectionState, rows: Int, cols: Int): SelectionState {
        val start = selection.start ?: return selection
        val end = selection.end ?: return selection
        val maxRow = (rows - 1).coerceAtLeast(0)
        val maxCol = (cols - 1).coerceAtLeast(0)
        return selection.copy(
            start = start.copy(
                row = start.row.coerceIn(0, maxRow),
                col = start.col.coerceIn(0, maxCol),
            ),
            end = end.copy(
                row = end.row.coerceIn(0, maxRow),
                col = end.col.coerceIn(0, maxCol),
            ),
        )
    }

    fun writeToPty(data: ByteArray) {
        val written = runtime.writeToPty(data)
        if (!written) {
            LogUtil.e("TerminalViewModel", "writeToPty failed for ${data.size} bytes")
        }
    }

    /** Feed bytes directly to the VT parser (test path for escape sequences). */
    fun feedTerminal(data: ByteArray) {
        runtime.feedTerminal(data)
    }

    fun cycleCtrlState() {
        _state.update { it.copy(ctrlState = it.ctrlState.next()) }
    }

    fun cycleAltState() {
        _state.update { it.copy(altState = it.altState.next()) }
    }

    fun consumeOneShotModifiers() {
        val currentState = _state.value
        var newCtrl = currentState.ctrlState
        var newAlt = currentState.altState
        if (newCtrl == ModifierState.Once) newCtrl = ModifierState.Off
        if (newAlt == ModifierState.Once) newAlt = ModifierState.Off
        if (newCtrl != currentState.ctrlState || newAlt != currentState.altState) {
            _state.update { current -> current.copy(ctrlState = newCtrl, altState = newAlt) }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3b: Selection management (extracted K5)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns selection state transitions: start/update/end, clipboard
     * copy/share, select-all and the extraction heuristics (URL joining,
     * TUI-border detection). Extracted from TerminalViewModel
     * inner class: accesses _state,
     * runtime and clipboardPaster via the outer view model.
     */
    inner class SelectionManager {
        fun startSelection(
            row: Int,
            col: Int,
            touchClass: TouchClass = TouchClass.Unknown,
        ) {
            val anchor = SelectionAnchor(row, col)
            // CAS: selection is touched from the main thread but
            // _state is also written by IO coroutines; a plain RMW could lose
            // one of their updates. mode is read inside the lambda. (The old
            // EmptyArea override to Char mode was dead — no caller passes
            // EmptyArea;.)
            _state.update { state ->
                state.copy(
                    selection =
                    SelectionState(
                        active = true,
                        dragging = true,
                        start = anchor,
                        end = anchor,
                        mode = state.selection.mode,
                        touchClass = touchClass,
                        menuDismissed = false,
                    ),
                )
            }
        }

        fun updateSelection(
            row: Int,
            col: Int,
        ) {
            // CAS with the active-check inside the lambda: the
            // selection read and the write are atomic against concurrent _state
            // updates. Paste-only selections (empty-cell/whitespace long-press)
            // are immutable — a finger micro-move during the long-press must
            // not drift the single cell.
            _state.update { state ->
                val current = state.selection
                if (!current.active || current.pasteOnly) {
                    state
                } else {
                    val result = current.applyHandleDrag(draggingStart = false, targetRow = row, targetCol = col)
                    state.copy(
                        selection =
                        current.copy(
                            dragging = true,
                            start = SelectionAnchor(result.startRow, result.startCol),
                            end = SelectionAnchor(result.endRow, result.endCol),
                            // Hide the floating menu while dragging; it
                            // reappears at the new position on ACTION_UP
                            // (endSelection restores menuDismissed=false).
                            menuDismissed = true,
                        ),
                    )
                }
            }
        }

        fun updateSelectionStart(
            row: Int,
            col: Int,
        ) {
            _state.update { state ->
                val current = state.selection
                if (!current.active || current.pasteOnly) {
                    state
                } else {
                    val result = current.applyHandleDrag(draggingStart = true, targetRow = row, targetCol = col)
                    state.copy(
                        selection =
                        current.copy(
                            dragging = true,
                            start = SelectionAnchor(result.startRow, result.startCol),
                            end = SelectionAnchor(result.endRow, result.endCol),
                            // Hide the floating menu while dragging; it
                            // reappears at the new position on ACTION_UP
                            // (endSelection restores menuDismissed=false).
                            menuDismissed = true,
                        ),
                    )
                }
            }
        }

        fun endSelection() {
            val current = _state.value.selection
            if (!current.active || current.start == null || current.end == null) return
            val text = extractSelectedText(current)
            // Only write when the selection is unchanged since our read: a
            // concurrent selection update must not be clobbered with stale text.
            // CAS loop: compareAndSet retries while the state is
            // still ours and aborts as soon as a concurrent write lands — the
            // native boundary sync below runs only for a genuinely committed
            // snapshot (no side-effect flag that could leak across retries).
            // Note: extractSelectedText itself reads the bridge and
            // cols across a snapshot — those can also go stale mid-extraction if
            // an IO coroutine switches sessions; the result is bounded by the
            // substring guards and is never written unless this CAS commits.
            val updated = current.copy(dragging = false, selectedText = text, menuDismissed = false)
            while (true) {
                val state = _state.value
                if (state.selection != current) return
                if (_state.compareAndSet(state, state.copy(selection = updated))) break
            }
            val start = current.start
            val end = current.end
            val loRow = minOf(start.row, end.row)
            val hiRow = maxOf(start.row, end.row)
            val loCol = minOf(start.col, end.col)
            val hiCol = maxOf(start.col, end.col)
            runtime.setSelection(loRow, loCol, hiRow, hiCol, true, current.mode.ordinal.toByte())
        }

        fun setSelectionMode(mode: SelectionMode) {
            _state.update { state ->
                val current = state.selection
                val adjusted =
                    if (!current.active || current.start == null || current.end == null) {
                        current
                    } else {
                        adjustSelectionForMode(current, mode)
                    }
                state.copy(selection = adjusted.copy(mode = mode))
            }
            syncSelectionToNative()
        }

        /**
         * mode switch re-expands the range (termlib
         * SelectionManager.adjustSelectionForMode:288-320): WORD expands
         * both ends onto word boundaries, LINE spans the full rows, CHAR
         * keeps the current range as-is.
         */
        private fun adjustSelectionForMode(
            selection: SelectionState,
            mode: SelectionMode,
        ): SelectionState {
            val start = selection.start ?: return selection
            val end = selection.end ?: return selection
            return when (mode) {
                SelectionMode.Line -> {
                    val cols = (runtime.state.value.cols - 1).coerceAtLeast(0)
                    selection.copy(
                        start = start.copy(col = 0),
                        end = end.copy(col = cols),
                    )
                }

                SelectionMode.Word -> {
                    val bridge = runtime.bridge() ?: return selection
                    val startLine = bridge.scrollbackLine(start.row) ?: return selection
                    val endLine = bridge.scrollbackLine(end.row) ?: return selection
                    val startWord = SelectionExpander.expandBounds(startLine, start.col)
                    val endWord = SelectionExpander.expandBounds(endLine, end.col)
                    selection.copy(
                        start = start.copy(col = startWord.first),
                        end = end.copy(col = endWord.second),
                    )
                }

                SelectionMode.Char,
                SelectionMode.Block,
                SelectionMode.Semantic,
                -> selection
            }
        }

        fun copySelectionToClipboard() {
            val rawText = _state.value.selection.selectedText
            if (rawText.isEmpty()) return
            // Smart processing, Haven smartCopy:357-405) applies
            // border-strip / wrapped-URL rebuild to the selection text; the
            // ClipboardAccess.smartCopyProcessor hook stays null here (OSC 52
            // programmatic writes from the runtime stay verbatim).
            val text = smartCopySelection(rawText)
            val clipped = if (text.length > CLIPBOARD_TEXT_MAX_LENGTH) text.substring(0, CLIPBOARD_TEXT_MAX_LENGTH) else text
            clipboardAccess.setClipboardText(clipped, label = "terminal selection")
            // Close the floating menu after the action; keep the highlight.
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }

        /**
         * route the copy through smart processing (TUI border
         * stripping + wrapped-URL rebuild, Haven smartCopy:357-405). Fetch
         * the selection's rows from the snapshot; on any bridge failure
         * fall back to the raw text.
         */
        private fun smartCopySelection(raw: String): String {
            val selection = _state.value.selection
            val start = selection.start ?: return raw
            val end = selection.end ?: return raw
            val bitmapSelection =
                if (start.row < end.row || (start.row == end.row && start.col <= end.col)) {
                    start to end
                } else {
                    end to start
                }
            val (lo, hi) = bitmapSelection
            val bridge = runtime.bridge() ?: return raw
            val lines =
                (lo.row..hi.row.coerceAtMost(lo.row + MAX_SELECTION_LINES))
                    .map { r -> bridge.scrollbackLine(r) ?: "" }
            if (lines.isEmpty()) return raw
            val text =
                SmartCopy.smartCopyText(
                    lines = lines,
                    startRow = 0,
                    startCol = lo.col.coerceAtLeast(0),
                    endRow = lines.size - 1,
                    endCol = hi.col.coerceAtLeast(0),
                    verbatim = raw,
                )
            return text.ifEmpty { raw }
        }

        fun clearSelection() {
            _state.update { it.copy(selection = SelectionState()) }
            syncSelectionToNative()
        }

        fun showPastePopup(
            row: Int,
            col: Int,
        ) {
            // an empty-cell long-press now creates a single-cell
            // selection (inverted background via the GPU path) with a
            // paste-only floating menu — matching the text-selection UX
            // instead of a detached chip with no highlight.
            _state.update { state ->
                state.copy(
                    selection =
                    SelectionState(
                        active = true,
                        dragging = false,
                        start = SelectionAnchor(row, col),
                        end = SelectionAnchor(row, col),
                        mode = SelectionMode.Char,
                        touchClass = TouchClass.EmptyArea,
                    ),
                )
            }
            syncSelectionToNative()
        }

        private fun syncSelectionToNative() {
            val selection = _state.value.selection
            if (selection.active && selection.start != null && selection.end != null) {
                val start = selection.start
                val end = selection.end
                val loRow = minOf(start.row, end.row)
                val hiRow = maxOf(start.row, end.row)
                val loCol = minOf(start.col, end.col)
                val hiCol = maxOf(start.col, end.col)
                runtime.setSelection(loRow, loCol, hiRow, hiCol, true, selection.mode.ordinal.toByte())
            } else {
                runtime.setSelection(0, 0, 0, 0, false, 0)
            }
        }

        fun shareSelection() {
            val rawText = _state.value.selection.selectedText
            if (rawText.isEmpty()) return
            // share the smart-copied text (border strip / URL rebuild).
            val text = smartCopySelection(rawText)
            val shareIntent =
                Intent.createChooser(
                    Intent(Intent.ACTION_SEND).apply {
                        type = "text/plain"
                        putExtra(Intent.EXTRA_TEXT, text)
                    },
                    null,
                )
            shareIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            context.startActivity(shareIntent)
            // Close the floating menu after the action.
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
        }

        fun selectAll(scrollOffset: Int = 0) {
            val runtimeState = runtime.state.value
            val rows = runtimeState.rows.coerceAtLeast(1)
            val cols = runtimeState.cols.coerceAtLeast(1)
            // Selection rows are grid rows (0 = top of scrollback). The
            // visible viewport starts at grid row (scrollbackLength - scrollOffset).
            val scrollbackLength = runtime.bridge()?.scrollbackLength() ?: 0
            val viewportStart = (scrollbackLength - scrollOffset).coerceAtLeast(0)
            val start = SelectionAnchor(row = viewportStart, col = 0)
            val end = SelectionAnchor(row = viewportStart + rows - 1, col = cols - 1)
            val selectionState =
                SelectionState(
                    active = true,
                    dragging = false,
                    start = start,
                    end = end,
                    mode = SelectionMode.Char,
                )
            val text = extractSelectedText(selectionState)
            _state.update { it.copy(selection = selectionState.copy(selectedText = text)) }
            syncSelectionToNative()
        }

        /**
         * Move the selection start anchor by [deltaCol] columns (character-by-character
         * navigation via d-pad buttons in the selection action bar).
         * Reference: research-haven.md §2.3 anchor movement buttons.
         */
        fun moveSelectionAnchor(deltaCol: Int) {
            val current = _state.value.selection
            val anchor = current.start ?: return
            val newCol = (anchor.col + deltaCol).coerceAtLeast(0)
            val newStart = anchor.copy(col = newCol)
            // Ensure start doesn't cross past end
            val end = current.end ?: return
            val clamped = if (
                newStart.row > end.row ||
                (newStart.row == end.row && newStart.col >= end.col)
            ) {
                anchor.copy(col = (end.col - 1).coerceAtLeast(0))
            } else {
                newStart
            }
            val updated = current.copy(start = clamped, menuDismissed = false)
            val text = extractSelectedText(updated)
            _state.update { it.copy(selection = updated.copy(selectedText = text)) }
            syncSelectionToNative()
        }

        /**
         * arrow-key selection movement — move the START anchor
         * by [deltaRow]/[deltaCol] (pure [SelectionState.moveSelectionAnchorBy]),
         * re-extract the selected text and sync to native. Called from
         * TerminalSurface.onKeyDown while a selection is active.
         */
        fun moveSelectionAnchorBy(deltaRow: Int, deltaCol: Int) {
            val selection = _state.value.selection
            if (!selection.active || selection.start == null || selection.end == null) return
            val runtimeState = runtime.state.value
            val maxRow = (runtimeState.rows - 1).coerceAtLeast(0)
            val maxCol = (runtimeState.cols - 1).coerceAtLeast(0)
            val updated = selection.moveSelectionAnchorBy(deltaRow, deltaCol, maxRow, maxCol)
            if (updated === selection) return
            val text = extractSelectedText(updated)
            _state.update { it.copy(selection = updated.copy(selectedText = text, menuDismissed = false)) }
            syncSelectionToNative()
        }

        @Suppress("CyclomaticComplexMethod")
        private fun extractSelectedText(selection: SelectionState): String {
            val start = selection.start ?: return ""
            val end = selection.end ?: return ""
            val bridge = runtime.bridge() ?: return ""
            // Selection rows are stored in grid coordinates (0 = top of
            // scrollback), matching the Ghostty formatter's grid rows.
            //
            // Wrap-aware extraction (termux TerminalBuffer.getSelectedText
            // semantics): soft-wrapped rows are joined without
            // '\n' (unwrap) and trailing whitespace is trimmed, and the
            // formatter maps grid columns to char indices internally so CJK
            // wide glyphs are never split (TerminalRow.findStartOfColumn
            // equivalent). The old per-row scrollbackLine + '\n' join could
            // not detect wraps and could cut surrogate pairs.
            val (lo, hi) =
                if (start.row < end.row || (start.row == end.row && start.col <= end.col)) {
                    start to end
                } else {
                    end to start
                }
            if (selection.mode == SelectionMode.Block) {
                // Block selection keeps its own rectangle semantics; the
                // formatter rectangle mode would pad lines, so extract
                // per-row substrings here (same as before).
                val visibleCols = runtime.state.value.cols.coerceAtLeast(1)
                val parts = mutableListOf<String>()
                for (r in lo.row..hi.row.coerceAtMost(lo.row + MAX_SELECTION_LINES)) {
                    val line = bridge.scrollbackLine(r) ?: ""
                    val visLine = if (line.length > visibleCols) line.substring(0, visibleCols) else line
                    var startCol = lo.col.coerceAtMost(visLine.length)
                    var endCol = hi.col.coerceAtMost(visLine.length)
                    if (startCol > endCol) {
                        val tmp = startCol
                        startCol = endCol
                        endCol = tmp
                    }
                    if (startCol < visLine.length) {
                        parts.add(extractBlockColumn(visLine, startCol, endCol))
                    }
                }
                return parts.joinToString("\n")
            }
            return bridge.selectionText(lo.row, lo.col, hi.row, hi.col, rectangle = false) ?: ""
        }

        private fun smartJoinLines(parts: List<String>): String {
            if (parts.size <= 1) return parts.joinToString("")
            val result = StringBuilder(parts[0])
            for (index in 1 until parts.size) {
                val previousLine = parts[index - 1]
                val currentLine = parts[index]
                if (isContinuationUrl(previousLine)) {
                    result.append(currentLine)
                } else if (isUrlStart(currentLine)) {
                    result.append("\n").append(currentLine)
                } else if (isPathOrProtocol(currentLine)) {
                    result.append(currentLine)
                } else if (isTuiBorder(currentLine)) {
                    break
                } else if (shouldJoinWithNewline(previousLine, currentLine)) {
                    result.append("\n").append(currentLine)
                } else {
                    result.append(currentLine)
                }
            }
            return result.toString()
        }

        private fun isContinuationUrl(line: String): Boolean = line.endsWith("https://") || line.endsWith("http://")

        private fun isUrlStart(line: String): Boolean = line.startsWith("https://") || line.startsWith("http://")

        private fun isPathOrProtocol(line: String): Boolean = line.startsWith("/") || line.startsWith("http")

        private fun shouldJoinWithNewline(
            previousLine: String,
            currentLine: String,
        ): Boolean {
            if (previousLine.isBlank() || currentLine.isBlank()) return false
            if (currentLine.startsWith(" ")) return false
            if (previousLine.endsWith(" ")) return false
            return true
        }

        private fun isTuiBorder(line: String): Boolean {
            val trimmed = line.trim()
            if (trimmed.isEmpty()) return false
            val uniqueChars = trimmed.toSet().size
            if (uniqueChars <= 2 && trimmed.all { it in "│─╭╮╰╯┌┐└┘┬┴├┤┼═║╗╝╚╔╠╣╦╩╬ " }) {
                return true
            }
            return false
        }

        private fun isWideChar(ch: Char): Boolean = isWideBmp(ch.code) || isWideAstral(ch.code)

        /** BMP wide ranges from Markus Kuhn's wcwidth() tables. */
        private fun isWideBmp(cp: Int): Boolean = cp in 0x1100..0x115F || // Hangul Jamo
            cp in 0x2329..0x232A || // angle brackets
            cp in 0x2E80..0x303E || // CJK Radicals .. CJK Symbols and Punctuation
            cp in 0x3041..0x33FF || // Hiragana .. CJK Compatibility
            cp in 0x3400..0x4DBF || // CJK Extension A
            cp in 0x4E00..0x9FFF || // CJK Unified Ideographs
            cp in 0xA000..0xA4CF || // Yi Syllables
            cp in 0xAC00..0xD7A3 || // Hangul Syllables
            cp in 0xF900..0xFAFF || // CJK Compatibility Ideographs
            cp in 0xFE30..0xFE4F || // CJK Compatibility Forms
            cp in 0xFF00..0xFF60 || // Fullwidth Forms
            cp in 0xFFE0..0xFFE6 // Fullwidth Signs

        /** Astral-plane wide ranges (emoji and CJK extensions B-G). */
        private fun isWideAstral(cp: Int): Boolean = cp in 0x1F1E6..0x1F1FF || // Regional Indicator (flag)
            cp in 0x1F300..0x1F64F || // Emoticons
            cp in 0x1F680..0x1F6FF || // Transport
            cp in 0x1F700..0x1F8FF || // Alchemical .. Geometric Extended
            cp in 0x1F900..0x1F9FF || // Supplemental Symbols
            cp in 0x1FA00..0x1FAFF || // Chess .. Symbols Extended-A
            cp in 0x20000..0x2FFFD || // CJK Extensions B-F
            cp in 0x30000..0x3FFFD // CJK Extension G

        /**
         * Extract a column-bounded rectangle slice from a single line,
         * correctly handling CJK wide characters that occupy 2 cell columns.
         */
        private fun extractBlockColumn(
            line: String,
            startCol: Int,
            endCol: Int,
        ): String {
            var col = 0
            var charStart = -1
            var charEnd = line.length
            for ((i, ch) in line.withIndex()) {
                val w = if (isWideChar(ch)) 2 else 1
                if (col >= startCol && charStart < 0) charStart = i
                if (col >= endCol) {
                    charEnd = i
                    break
                }
                col += w
            }
            if (charStart < 0) return ""
            return line.substring(charStart, charEnd.coerceAtMost(line.length))
        }

        /** Paste clipboard content directly to the PTY (no confirmation dialog). */
        fun pasteFromClipboard(): Int {
            val text = clipboardAccess.clipboardText() ?: return 0
            return executePaste(text)
        }

        /** Actually send [text] to PTY via the chunker. */
        fun executePaste(text: String): Int {
            var offset = 0
            for (chunk in PasteChunker().chunks(text)) {
                runtime.writeToPty(chunk.toByteArray())
                offset += chunk.length
            }
            _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
            return offset
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2b: Font management (extracted R4)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns font loading, size/family settings and font-file installation.
     * Extracted from TerminalViewModel.
     * Inner class: accesses the font StateFlows, runtime, settingsRepository
     * and context via the outer view model. loadFonts() refreshes the flows
     * after install.
     */
    inner class FontManager {
        fun loadFonts() {
            viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
                try {
                    val bridge = runtime.bridge()
                    val rustFontFamilies = bridge?.listFontFamilies() ?: emptyList()
                    val fileSystemFonts = terminal.emulator.settings.systemFonts()
                    val userFonts =
                        try {
                            val allUserFonts = mutableListOf<String>()
                            val filesDir = context.filesDir.resolve("fonts")
                            if (filesDir.isDirectory) {
                                allUserFonts.addAll(
                                    filesDir
                                        .listFiles()
                                        ?.filter { it.isFile && (it.extension == "ttf" || it.extension == "otf") }
                                        ?.map { it.nameWithoutExtension }
                                        ?: emptyList(),
                                )
                            }
                            val cacheDir = context.cacheDir.resolve("fonts")
                            if (cacheDir.isDirectory) {
                                cacheDir
                                    .listFiles()
                                    ?.filter { it.isFile && (it.extension == "ttf" || it.extension == "otf") }
                                    ?.forEach { cachedFile ->
                                        val destFile = java.io.File(filesDir, cachedFile.name)
                                        if (!destFile.exists()) {
                                            cachedFile.copyTo(destFile)
                                        }
                                        cachedFile.delete()
                                        allUserFonts.add(cachedFile.nameWithoutExtension)
                                    }
                                if (cacheDir.listFiles().isNullOrEmpty()) {
                                    cacheDir.delete()
                                }
                            }
                            allUserFonts.distinct()
                        } catch (exception: Exception) {
                            Log.e(TAG, "Failed to load user fonts", exception)
                            emptyList()
                        }
                    val allFonts =
                        (rustFontFamilies + fileSystemFonts + userFonts)
                            .distinct()
                            .sorted()
                    _availableFonts.value = allFonts
                    _defaultFontName.value = bridge?.getDefaultFontName() ?: fileSystemFonts.firstOrNull() ?: ""
                    _fontInfo.value =
                        bridge?.getFontInfo() ?: FontInfoDto.placeholderJson(_defaultFontName.value)
                } catch (exception: Exception) {
                    Log.e("TerminalViewModel", "Failed to load font list", exception)
                    _availableFonts.value = emptyList()
                }
            }
        }

        fun setFontSize(size: Float) {
            viewModelScope.launch(Dispatchers.IO) {
                settingsRepository.setFontSize(size)
                runtime.applyFontSettings()
                val bridge = runtime.bridge()
                if (bridge != null) {
                    _fontInfo.value = bridge.getFontInfo() ?: context.getString(R.string.no_font_loaded)
                }
            }
        }

        fun setFontFamily(family: String) {
            viewModelScope.launch(Dispatchers.IO) {
                try {
                    android.util.Log.d("Font", "Setting font family: $family")
                    settingsRepository.setFontFamily(family)
                    runtime.applyFontSettings()
                    val bridge = runtime.bridge()
                    val fontName = bridge?.getDefaultFontName() ?: "monospace"
                    val fontInfo = bridge?.getFontInfo() ?: context.getString(R.string.no_font_loaded)
                    _defaultFontName.value = fontName
                    _fontInfo.value = fontInfo
                    android.util.Log.d("Font", "Font applied: $fontName")
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(context, context.getString(R.string.font_applied, fontName), android.widget.Toast.LENGTH_SHORT)
                            .show()
                    }
                } catch (exception: Exception) {
                    android.util.Log.e("Font", "setFontFamily failed for $family", exception)
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(
                                context,
                                context.getString(R.string.font_apply_failed, exception.message ?: ""),
                                android.widget.Toast.LENGTH_SHORT,
                            ).show()
                    }
                }
            }
        }

        /**
         * Set the independent family for a style slot (0=bold, 1=italic,
         * 2=bold-italic) — ghostty-android TerminalFontStore 4-slot design
         * research-ghostty-android-extra.md:80). Empty clears the slot.
         */
        fun setFontFamilyForStyle(family: String, slot: Int) {
            viewModelScope.launch(Dispatchers.IO) {
                try {
                    android.util.Log.d("Font", "Setting style slot $slot font family: $family")
                    when (slot) {
                        FONT_SLOT_REGULAR -> settingsRepository.setFontFamily(family)
                        FONT_SLOT_BOLD -> settingsRepository.setBoldFontFamily(family)
                        FONT_SLOT_ITALIC -> settingsRepository.setItalicFontFamily(family)
                        else -> Unit
                    }
                    runtime.applyFontSettings()
                    val bridge = runtime.bridge()
                    val fontName = bridge?.getDefaultFontName() ?: "monospace"
                    _defaultFontName.value = fontName
                    android.util.Log.d("Font", "Style slot $slot font applied")
                } catch (exception: Exception) {
                    android.util.Log.e("Font", "setFontFamilyForStyle failed for slot $slot", exception)
                }
            }
        }

        fun installFontFile(uri: Uri) {
            viewModelScope.launch(Dispatchers.IO) {
                try {
                    val rawName = getFileNameFromUri(uri) ?: uri.lastPathSegment ?: "custom_font.ttf"
                    // Sanitize: DISPLAY_NAME from a content provider may contain
                    // path separators or "..", which would escape fontsDir and
                    // overwrite app-private files.
                    val fileName = sanitizeFontFileName(rawName)
                    val fontsDir =
                        context.filesDir.resolve("fonts").also { dir ->
                            if (!dir.mkdirs()) {
                                Log.w("TerminalViewModel", "Failed to create fonts directory: $dir")
                            }
                        }
                    val destFile = java.io.File(fontsDir, fileName)
                    context.contentResolver.openInputStream(uri)?.use { input ->
                        destFile.outputStream().use { output ->
                            input.copyTo(output)
                        }
                    } ?: run {
                        kotlinx.coroutines.withContext(Dispatchers.Main) {
                            android.widget.Toast
                                .makeText(context, context.getString(R.string.font_read_failed), android.widget.Toast.LENGTH_SHORT)
                                .show()
                        }
                        return@launch
                    }

                    android.util.Log.d("Font", "Font file copied: ${destFile.absolutePath} (${destFile.length()} bytes)")

                    val familyName = runtime.loadFontFile(destFile.absolutePath)
                    if (familyName != null) {
                        android.util.Log.d("Font", "Font loaded: family=$familyName")
                        settingsRepository.setFontFamily(familyName)
                        runtime.applyFontSettings()
                        loadFonts()
                        kotlinx.coroutines.withContext(Dispatchers.Main) {
                            android.widget.Toast
                                .makeText(context, context.getString(R.string.font_installed, familyName), android.widget.Toast.LENGTH_SHORT)
                                .show()
                        }
                    } else {
                        android.util.Log.e("Font", "Font load failed: null family from ${destFile.absolutePath}")
                        kotlinx.coroutines.withContext(Dispatchers.Main) {
                            android.widget.Toast
                                .makeText(context, context.getString(R.string.font_not_supported), android.widget.Toast.LENGTH_SHORT)
                                .show()
                        }
                    }
                } catch (exception: Exception) {
                    android.util.Log.e("TerminalViewModel", "installFontFile failed", exception)
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(
                                context,
                                context.getString(R.string.font_install_failed, exception.message ?: ""),
                                android.widget.Toast.LENGTH_SHORT,
                            ).show()
                    }
                }
            }
        }

        fun getFileNameFromUri(uri: Uri): String? {
            val cursor = context.contentResolver.query(uri, null, null, null, null)
            return cursor?.use {
                if (it.moveToFirst()) {
                    val index = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (index >= 0) it.getString(index) else null
                } else {
                    null
                }
            }
        }

        fun sanitizeFontFileName(name: String): String {
            val base = name.substringAfterLast('/').substringAfterLast('\\')
            if (base.isEmpty() || base == "." || base == "..") return "custom_font.ttf"
            return base
        }
    }

    companion object {
        private const val TAG = "TerminalViewModel"
        private const val STOP_TIMEOUT_MILLIS = 5000L
        private const val DEBOUNCE_MILLIS = 300L

        // Upper bound for text extraction loops (one JNI scrollbackLine call
        // per row, on the main thread). Keeps worst case bounded even when a
        // broadcast-injected selection claims thousands of rows.
        private const val MAX_SELECTION_LINES = 2000

        // Upper bound for clipboard paste (main-thread string copies) and
        // the chunk size used to stream it (must stay well below the PTY
    }

    private val _state = MutableStateFlow(TerminalState())
    val state: StateFlow<TerminalState> = _state.asStateFlow()

    @Volatile var currentSurface: Surface? = null

    @Volatile var surfaceWidth: Int = 0

    @Volatile var surfaceHeight: Int = 0

    fun startRuntime(
        surface: Surface?,
        width: Int,
        height: Int,
    ) {
        currentSurface = surface
        surfaceWidth = width
        surfaceHeight = height
        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            runtime.start(surface, width, height)
        }
    }

    /**
     * Single merged snapshot of every persisted setting (C7). UI subscribes
     * to this one StateFlow; per-field access is `settings.fontSize` etc.
     */
    val settings: StateFlow<SettingsRepository.SettingsState> =
        settingsRepository.settings
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS),
                // Match the device-adaptive default so the pre-flow snapshot
                // never flashes the fixed 10sp fallback.
                SettingsRepository.SettingsState(
                    fontSize =
                    SettingsRepository.defaultFontSizeFor(
                        context.resources.displayMetrics.widthPixels /
                            context.resources.displayMetrics.density,
                    ),
                ),
            )

    private val _availableFonts = MutableStateFlow<List<String>>(emptyList())
    val availableFonts: StateFlow<List<String>> = _availableFonts.asStateFlow()

    private val _defaultFontName = MutableStateFlow("")
    val defaultFontName: StateFlow<String> = _defaultFontName.asStateFlow()

    private val _fontInfo = MutableStateFlow(FontInfoDto.placeholderJson("monospace"))
    val fontInfo: StateFlow<String> = _fontInfo.asStateFlow()

    init {
        // First launch: pin a device-adaptive default font size so the grid
        // is legible before the user touches the font-size slider.
        viewModelScope.launch {
            val metrics = context.resources.displayMetrics
            settingsRepository.applyFirstLaunchDefaultFontSize(metrics.widthPixels / metrics.density)
        }
        // Load persisted shortcut bindings from DataStore  fix).
        viewModelScope.launch {
            settings.map {
                it.shortcutPaste to it.shortcutNewSession to
                    it.shortcutCloseSession to it.shortcutCopy to it.shortcutToggleScroll
            }
                .distinctUntilChanged()
                .collect { loadShortcutBindingsFromDataStore(settings.value) }
        }
        viewModelScope.launch {
            runtime.state.collect { runtimeState ->
                // a grid resize can shrink below the current
                // selection bounds; clamp start/end onto the new grid so
                // native setSelection never sees out-of-bounds cells.
                if (runtimeState.rows != lastGridRows || runtimeState.cols != lastGridCols) {
                    lastGridRows = runtimeState.rows
                    lastGridCols = runtimeState.cols
                    _state.update { current ->
                        current.copy(
                            selection = clampSelectionToGrid(current.selection, runtimeState.rows, runtimeState.cols),
                        )
                    }
                }
                val sortedIds = runtimeState.sessionIds.sorted()
                val sessions =
                    sortedIds.mapIndexed { index, id ->
                        SessionInfo(id = id, title = context.getString(R.string.session_number, index + 1))
                    }
                val active = runtimeState.activeSessionId
                if (active != 0L) {
                    val displayIndex = sortedIds.indexOf(active) + 1
                    val title =
                        if (runtimeState.title.isNotEmpty()) {
                            runtimeState.title
                        } else {
                            context.getString(R.string.session_number, displayIndex)
                        }
                    // _state.update (CAS) instead of read-modify-write:
                    // createSession/switchSession on the IO dispatcher also
                    // update _state, and a non-atomic write here could
                    // clobber their just-committed session list.
                    _state.update { current ->
                        current.copy(
                            sessionId = active,
                            isRunning = runtimeState.isRunning,
                            title = title,
                            sessions = sessions,
                            activeSessionId = active,
                            selectionBg = runtime.selectionBgColor,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                    if (runtime.state.value.sessionIds
                            .isNotEmpty()
                    ) {
                        if (_availableFonts.value.isEmpty()) {
                            fontManager.loadFonts()
                        } else {
                            val bridge = runtime.bridge()
                            _defaultFontName.value = bridge?.getDefaultFontName() ?: ""
                            _fontInfo.value = bridge?.getFontInfo() ?: context.getString(R.string.no_font_loaded)
                        }
                    }
                } else {
                    _state.update { current ->
                        current.copy(sessions = sessions, activeSessionId = active)
                    }
                }
            }
        }
        viewModelScope.launch {
            settingsRepository.keyboardMode.collect { mode ->
                _state.update { it.copy(keyboardMode = mode.toKeyboardMode()) }
            }
        }
        viewModelScope.launch {
            val savedPath = settingsRepository.backgroundImagePath.first()
            if (savedPath.isNotEmpty()) {
                applyBackgroundImageFromPath(savedPath)
            }
            // Same cold-start race as applyBackgroundImageFromPath: the
            // bridge may not exist yet, and dropping the params here would
            // leave blur/alpha at their defaults until the user touches
            // the sliders, emulator-verified).
            val bridge = withTimeoutOrNull(15_000) {
                while (runtime.bridge() == null) {
                    delay(100)
                }
                runtime.bridge()
            }
            settingsRepository.backgroundBlurRadius.first().let { radius ->
                settingsRepository.backgroundAlpha.first().let { alpha ->
                    bridge?.setBackgroundParams(radius, (alpha * 10).toInt())
                }
            }
        }
        viewModelScope.launch {
            settings.map { it.cursorBlink }.distinctUntilChanged().collect { enabled ->
                val bridge = runtime.bridge() ?: return@collect
                bridge.setCursorBlinkEnabled(enabled)
                runtime.forceRender()
            }
        }
        viewModelScope.launch {
            settings.map { it.cursorSpeed }.distinctUntilChanged().collect { speed ->
                val bridge = runtime.bridge() ?: return@collect
                bridge.setCursorBlinkSpeedMs(speed.coerceIn(100, 1000))
                runtime.forceRender()
            }
        }
    }

    /**
     * Delete all app-private data (settings, sessions, logs, cache) and
     * recreate the DataStore prefs directory so the next settings write
     * does not fail (C10: moved out of the settings UI composable).
     *
     * The [onComplete] callback runs on the IO dispatcher (not the main
     * thread); post UI work (e.g. Toasts) must hop to the main thread
     * themselves or use Android's auto-posting Toast API.
     */
    fun clearAppData(onComplete: () -> Unit) {
        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                context.getDir("prefs", Context.MODE_PRIVATE).deleteRecursively()
                context.getDir("sessions", Context.MODE_PRIVATE).deleteRecursively()
                context.getDir("logs", Context.MODE_PRIVATE).deleteRecursively()
                context.getDir("logs_root", Context.MODE_PRIVATE).deleteRecursively()
                context.getDir("bin", Context.MODE_PRIVATE).deleteRecursively()
                context.cacheDir.listFiles()?.forEach { it.delete() }
                // The process-wide DataStore singleton keeps running: recreate
                // the prefs directory so the next settings write does not fail.
                context.getDir("prefs", Context.MODE_PRIVATE)
            } catch (exception: Exception) {
                Log.e("ClearAppData", "Failed to clear app data", exception)
            } finally {
                onComplete()
            }
        }
    }

    fun resetCursorBlink() {
        val bridge = runtime.bridge() ?: return
        bridge.resetCursorBlink()
        runtime.forceRender()
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2: Session orchestration & settings setters
    // ══════════════════════════════════════════════════════════════════════

    fun ensureDefaultSession() {
        if (!shouldCreateDefaultSession(
                surfaceValid = currentSurface?.isValid == true,
                surfaceWidth = surfaceWidth,
                surfaceHeight = surfaceHeight,
                uiSessions = _state.value.sessions,
                runtimeSessionIds = runtime.state.value.sessionIds,
            )
        ) {
            return
        }
        android.util.Log.d("TerminalViewModel", "ensureDefaultSession: creating default session")
        createSession()
    }

    fun setBootstrapUrl(url: String) {
        bootstrapUrlDebounce.value = url
        bootstrapUrlEdited = true
    }

    private val _bootstrapRunning = MutableStateFlow(false)
    val bootstrapRunning: StateFlow<Boolean> = _bootstrapRunning.asStateFlow()

    private val _bootstrapResult = MutableStateFlow<String?>(null)
    val bootstrapResult: StateFlow<String?> = _bootstrapResult.asStateFlow()

    private val _bootstrapProgress = MutableStateFlow<terminal.emulator.installer.BootstrapProgress?>(null)
    val bootstrapProgress: StateFlow<terminal.emulator.installer.BootstrapProgress?> =
        _bootstrapProgress.asStateFlow()

    /** Maps a bootstrap result to localized user text; failure payloads are
     *  machine-readable keys (see [BootstrapOrchestrator]). */
    private fun bootstrapOutcomeText(result: Result<String>): String = result.fold(
        onSuccess = { diagnostics ->
            val headline = context.getString(R.string.bootstrap_installed_success)
            if (diagnostics.isEmpty()) {
                headline
            } else {
                context.getString(R.string.bootstrap_postinst_errors) + "\n" + diagnostics
            }
        },
        onFailure = { exception ->
            when (exception.message) {
                terminal.emulator.installer.BootstrapOrchestrator.ERROR_PRIMARY_USER_REQUIRED ->
                    context.getString(R.string.bootstrap_primary_user_required)

                terminal.emulator.installer.BootstrapOrchestrator.ERROR_ALREADY_IN_PROGRESS ->
                    context.getString(R.string.bootstrap_already_in_progress)

                terminal.emulator.installer.BootstrapOrchestrator.ERROR_NO_URL ->
                    context.getString(R.string.bootstrap_no_url)

                terminal.emulator.installer.BootstrapOrchestrator.ERROR_CANCELLED ->
                    context.getString(R.string.bootstrap_cancelled)

                else -> context.getString(R.string.bootstrap_error, exception.javaClass.simpleName)
            }
        },
    )

    fun runBootstrap() {
        // CAS so a rapid double-tap of the Install button cannot start two
        // concurrent installs.
        if (!_bootstrapRunning.compareAndSet(false, true)) return
        viewModelScope.launch(Dispatchers.IO) {
            _bootstrapResult.value = null
            _bootstrapProgress.value = null
            try {
                val onProgress =
                    terminal.emulator.installer.BootstrapProgressCallback { progress ->
                        _bootstrapProgress.value = progress
                    }
                val downloader =
                    terminal.emulator.installer.BootstrapDownloader(
                        context,
                        onProgress = onProgress,
                    )
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "usr-staging"),
                        onProgress = onProgress,
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        onProgress = onProgress,
                    )
                val orchestrator =
                    terminal.emulator.installer.BootstrapOrchestrator(
                        downloader,
                        installer,
                        secondStage,
                        onProgress = onProgress,
                    )
                // Read the debounced value directly: the DataStore write is
                // debounced by 500ms, so first() could still return the old
                // URL if the user taps Install right after typing. An edited
                // (even if cleared) field wins over the stored URL.
                val url =
                    if (bootstrapUrlEdited) {
                        bootstrapUrlDebounce.value
                    } else {
                        settingsRepository.bootstrapUrl.first()
                    }
                val result = orchestrator.ensureBootstrap(url)
                _bootstrapResult.value = bootstrapOutcomeText(result)
            } catch (exception: Exception) {
                _bootstrapResult.value = context.getString(R.string.bootstrap_error, exception.javaClass.simpleName)
            } finally {
                _bootstrapRunning.value = false
            }
        }
    }

    /**
     * Offline bootstrap install from a SAF URI.  The user picks a.zip file
     * via [android.activity.result.contract.ActivityResultContracts.OpenDocument];
     * the content is copied to a cache file, then fed to the same installer
     * pipeline as the online path (installer.install → secondStage.run).
     * No network required; the downloaded bootstrap URL is ignored.
     */
    fun installOffline(uri: android.net.Uri) {
        if (!_bootstrapRunning.compareAndSet(false, true)) return
        viewModelScope.launch(Dispatchers.IO) {
            _bootstrapResult.value = null
            _bootstrapProgress.value = null
            try {
                val onProgress =
                    terminal.emulator.installer.BootstrapProgressCallback { progress ->
                        _bootstrapProgress.value = progress
                    }
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "usr-staging"),
                        onProgress = onProgress,
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        onProgress = onProgress,
                    )
                // Copy SAF URI content to a temp cache file — the installer
                // needs a File (it hashes + streams the zip).  The cache dir
                // is always writable and cleared by the OS under pressure.
                _bootstrapProgress.value = terminal.emulator.installer.BootstrapProgress.Downloading(0, 0)
                val cacheFile = java.io.File(context.cacheDir, "offline-bootstrap.zip")
                context.contentResolver.openInputStream(uri)?.use { input ->
                    cacheFile.outputStream().use { output ->
                        input.copyTo(output)
                    }
                } ?: throw java.io.IOException("Failed to open bootstrap file")
                val result = installer.install(cacheFile)
                if (result.isSuccess) {
                    _bootstrapProgress.value = terminal.emulator.installer.BootstrapProgress.Complete
                    val secondResult = secondStage.run()
                    val headline = context.getString(R.string.bootstrap_installed_from_file)
                    val diagnostics = secondResult.errors.take(3).joinToString("\n") { "- $it" }
                    val details =
                        if (secondResult.errors.isNotEmpty()) {
                            context.getString(R.string.bootstrap_postinst_errors) + "\n" + diagnostics
                        } else {
                            ""
                        }
                    _bootstrapResult.value =
                        if (details.isEmpty()) headline else "$headline\n$details"
                } else {
                    _bootstrapResult.value = context.getString(R.string.bootstrap_error, result.exceptionOrNull()?.javaClass?.simpleName)
                }
                cacheFile.delete()
            } catch (exception: Exception) {
                LogUtil.e("ViewModel", "Offline install failed", exception)
                _bootstrapResult.value = context.getString(R.string.bootstrap_error, exception.javaClass.simpleName)
            } finally {
                _bootstrapRunning.value = false
            }
        }
    }

    fun setUseNerdFontGlyphs(enabled: Boolean) {
        viewModelScope.launch {
            settingsRepository.setUseNerdFontGlyphs(enabled)
        }
    }

    fun setKeyboardMode(mode: KeyboardMode) {
        viewModelScope.launch {
            settingsRepository.setKeyboardMode(mode.toSettingsString())
        }
    }

    fun setMcpServerEnabled(enabled: Boolean) {
        // Native stop() may join the server thread for up to ~50ms; never
        // run it on the main thread from the settings screen.
        viewModelScope.launch(Dispatchers.IO) {
            if (NativeBridge.isNativeLoaded()) {
                NativeBridge.setMcpEnabled(enabled)
            } else {
                // Native library missing: setMcpEnabled is an external fun and
                // would throw UnsatisfiedLinkError (an Error, not an Exception),
                // crashing the app from the settings screen.
                Log.w("TerminalViewModel", "setMcpServerEnabled: native library not loaded, ignoring toggle")
            }
            settingsRepository.setMcpServerEnabled(enabled)
        }
    }

    fun setEnvironmentVariables(vars: Map<String, String>) {
        // Applied on the next session spawn (native side folds them into
        // the PTY environment); no live-session mutation.
        viewModelScope.launch {
            settingsRepository.setEnvironmentVariables(vars)
        }
    }

    fun setBackgroundImagePath(path: String) {
        viewModelScope.launch(Dispatchers.IO) {
            // ghostty-android BackgroundImageStore pattern: copy the picked
            // image into app-private storage so later reads never depend on
            // a content-provider grant (SAF persistable permissions can be
            // revoked; the private copy survives). The stored path points at
            // the private file; a missing file is treated as "no wallpaper"
            // on restore (self-heal, see applyBackgroundImageFromPath).
            var effectivePath = path
            if (path.startsWith("content://")) {
                val dst = java.io.File(context.filesDir, "terminal_background")
                if (copyContentUriToPrivateFile(context.contentResolver, path.toUri(), dst)) {
                    effectivePath = dst.absolutePath
                    LogUtil.d(TAG, "background image copied to private storage: $effectivePath")
                } else {
                    LogUtil.e(TAG, "background image private copy failed, keeping original path")
                }
            }
            settingsRepository.setBackgroundImagePath(effectivePath)
            applyBackgroundImageFromPath(effectivePath)
        }
    }

    private fun applyBackgroundImageFromPath(path: String) {
        viewModelScope.launch(Dispatchers.IO) {
            // The first session may still be spawning (bootstrap install,
            // shell start) when this runs after a cold start — `bridge()`
            // is null then and the wallpaper would be silently dropped.
            // Wait up to 15s for a bridge (emulator-verified:
            // after relaunch the DataStore path survived but the image was
            // never applied because the bridge was not ready yet).
            val bridge = withTimeoutOrNull(15_000) {
                while (runtime.bridge() == null) {
                    delay(100)
                }
                runtime.bridge()
            } ?: return@launch
            if (path.isNotEmpty()) {
                try {
                    val uri = path.toUri()
                    // Coil decodes with EXIF orientation applied and a
                    // size cap (1920x1080) that preserves the previous
                    // inSampleSize OOM protection. OutOfMemoryError is an
                    // Error, not an Exception, so it must be caught too.
                    val imageLoader = ImageLoader.Builder(context).build()
                    val request =
                        ImageRequest.Builder(context)
                            .data(uri)
                            .size(1920, 1080)
                            // Coil 3 returns HARDWARE bitmaps by default;
                            // copyPixelsToBuffer() below throws
                            // IllegalStateException on those. Force a
                            // software bitmap so the RGBA bytes can be
                            // read, emulator-verified).
                            .allowHardware(false)
                            .build()
                    val image = imageLoader.execute(request).image
                    val bitmap = (image as? BitmapImage)?.bitmap
                    if (bitmap != null) {
                        val bitmapWidth = bitmap.width
                        val bitmapHeight = bitmap.height
                        val buffer = java.nio.ByteBuffer.allocate(bitmapWidth * bitmapHeight * 4)
                        bitmap.copyPixelsToBuffer(buffer)
                        val rgbaData = buffer.array()
                        // Byte order note, emulator-verified):
                        // Coil 3 with.allowHardware(false) returns a bitmap
                        // whose copyPixelsToBuffer() output is already RGBA
                        // byte order on this pipeline. The earlier BGRA→RGBA
                        // swap  double-swapped and rendered the
                        // wallpaper with red/blue exchanged (quadrant-color
                        // pixel checks). No swap is applied here.
                        bridge.setBackgroundImage(rgbaData, bitmapWidth, bitmapHeight)
                        bridge.setBackgroundParams(
                            settings.value.backgroundBlurRadius,
                            (settings.value.backgroundAlpha * 10).toInt(),
                        )
                    }
                } catch (e: Throwable) {
                    // Self-heal (ghostty-android BackgroundImageStore decode()
                    // returns null for a missing/undecodable file): if the
                    // stored image is gone, clear the setting so the UI shows
                    // "no wallpaper" and the solid theme background renders.
                    Log.e(TAG, "setBackgroundImagePath failed", e)
                    if (path.startsWith(context.filesDir.absolutePath)) {
                        val file = java.io.File(path)
                        if (!file.exists()) {
                            LogUtil.w(TAG, "background image file missing — clearing setting (self-heal)")
                            settingsRepository.setBackgroundImagePath("")
                        }
                    }
                }
            } else {
                bridge.clearBackgroundImage()
            }
        }
    }

    fun setBackgroundBlurRadius(radius: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            settingsRepository.setBackgroundBlurRadius(radius)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setBackgroundParams(radius, (settings.value.backgroundAlpha * 10).toInt())
        }
    }

    fun setBackgroundAlpha(alpha: Float) {
        viewModelScope.launch(Dispatchers.IO) {
            settingsRepository.setBackgroundAlpha(alpha)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setBackgroundParams(settings.value.backgroundBlurRadius, (alpha * 10).toInt())
        }
    }

    fun setCursorBlink(enabled: Boolean) = applyCursorSetting({ settingsRepository.setCursorBlink(enabled) }) { bridge ->
        bridge.setCursorBlinkEnabled(enabled)
    }

    fun setCursorSpeed(speedMs: Int) = applyCursorSetting({ settingsRepository.setCursorSpeed(speedMs) }) { bridge ->
        bridge.setCursorBlinkSpeedMs(speedMs.coerceIn(100, 1000))
    }

    fun setCursorStyle(style: String) = applyCursorSetting({ settingsRepository.setCursorStyle(style) }) { bridge ->
        bridge.setCursorStyle(style)
    }

    fun setBellMode(modeId: Int) {
        viewModelScope.launch {
            settingsRepository.setBellMode(modeId)
        }
    }

    /**
     * Persist a cursor setting then push it to the bridge and force a
     * render. Shared by the three cursor setters (R10:
     * architecture).
     */
    private fun applyCursorSetting(
        persist: suspend () -> Unit,
        applyToBridge: (terminal.emulator.bridge.Bridge) -> Unit,
    ) {
        viewModelScope.launch {
            persist()
            val bridge = runtime.bridge() ?: return@launch
            applyToBridge(bridge)
            runtime.forceRender()
        }
    }

    fun setThemeName(name: String) = applyThemeSettings { settingsRepository.setThemeName(name) }

    // User-created themes (ghostty-android ThemeStore pattern): save the
    // current resolved theme under a new name, or delete a saved user theme.
    // Persisted in DataStore via [UserThemeStore]; a name collision replaces
    // the existing entry.
    init {
        viewModelScope.launch {
            userThemeStore.userThemes.collect { _userThemes.value = it }
        }
    }

    fun saveCurrentThemeAs(name: String, isDark: Boolean) {
        if (name.isBlank()) return
        viewModelScope.launch {
            val mode = settingsRepository.themeMode.first()
            val resolved =
                resolveTerminalThemeName(
                    mode,
                    settingsRepository.themeName.first(),
                    settingsRepository.dayThemeName.first(),
                    settingsRepository.nightThemeName.first(),
                    isDark,
                )
            val current = BuiltInThemes.byName(resolved)
            userThemeStore.save(current.copy(name = name.trim()))
        }
    }

    fun deleteUserTheme(name: String) {
        viewModelScope.launch {
            userThemeStore.delete(name)
            // If the deleted theme was selected, fall back to the default.
            val current = settingsRepository.themeName.first()
            if (current == name) settingsRepository.setThemeName("Dracula Plus")
        }
    }

    /**
     * Overwrite an existing user theme with a new definition from the
     * theme editor. The name must match an existing user theme.
     */
    fun overwriteUserTheme(theme: terminal.emulator.ui.theme.TerminalTheme) {
        viewModelScope.launch {
            userThemeStore.save(theme)
        }
    }

    /**
     * Save an edited theme as a brand-new user theme.
     */
    fun saveEditedThemeAsNew(name: String, theme: terminal.emulator.ui.theme.TerminalTheme) {
        if (name.isBlank()) return
        viewModelScope.launch {
            userThemeStore.save(theme.copy(name = name.trim()))
        }
    }

    fun setDayThemeName(name: String) = applyThemeSettings { settingsRepository.setDayThemeName(name) }

    fun setNightThemeName(name: String) = applyThemeSettings { settingsRepository.setNightThemeName(name) }

    fun setThemeMode(mode: String) = applyThemeSettings { settingsRepository.setThemeMode(mode) }

    fun setAppThemeMode(mode: String) = applyThemeSettings { settingsRepository.setAppThemeMode(mode) }

    /**
     * Persist a theme setting then re-apply the whole theme to the bridge.
     * Shared by the five theme setters (R10:  architecture).
     */
    private fun applyThemeSettings(persist: suspend () -> Unit) {
        viewModelScope.launch {
            persist()
            runtime.applySettings()
        }
    }

    /** Keep only the final path segment and reject any traversal. */

    private val shellTextDebounce = MutableStateFlow("")
    private val bootstrapUrlDebounce = MutableStateFlow("")

    // Written on the UI thread, read on an IO coroutine; volatile makes the
    // visibility explicit instead of relying on implicit happens-before
    //
    @Volatile
    private var bootstrapUrlEdited = false

    init {
        // Debounce free-text settings so typing does not write DataStore
        // on every keystroke (each write is a full file rewrite).
        @OptIn(kotlinx.coroutines.FlowPreview::class)
        viewModelScope.launch {
            shellTextDebounce
                .debounce(DEBOUNCE_MILLIS)
                .distinctUntilChanged()
                .collect { value -> settingsRepository.setShell(value) }
        }
        @OptIn(kotlinx.coroutines.FlowPreview::class)
        viewModelScope.launch {
            bootstrapUrlDebounce
                .debounce(DEBOUNCE_MILLIS)
                .distinctUntilChanged()
                .collect { value -> settingsRepository.setBootstrapUrl(value) }
        }
    }

    fun setShell(shell: String) {
        shellTextDebounce.value = shell
    }

    fun setScrollbackLines(lines: Int) {
        viewModelScope.launch {
            settingsRepository.setScrollbackLines(lines)
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4: Keyboard & hardware-key handling

    fun handleLayoutAwareHardwareKey(event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false

        // Only physical keyboards. Soft-keyboard / IME input flows through the
        // InputConnection (commitText); intercepting it here would turn CJK /
        // voice composition into raw Latin letters.
        if ((event.flags and KeyEvent.FLAG_SOFT_KEYBOARD) != 0) return false
        if (event.deviceId == KeyCharacterMap.VIRTUAL_KEYBOARD) return false
        if (!event.isFromSource(InputDevice.SOURCE_KEYBOARD)) return false

        val keyCode = event.keyCode
        // Skip modifier-only presses and let the view handle them.
        when (keyCode) {
            KeyEvent.KEYCODE_SHIFT_LEFT,
            KeyEvent.KEYCODE_SHIFT_RIGHT,
            KeyEvent.KEYCODE_CTRL_LEFT,
            KeyEvent.KEYCODE_CTRL_RIGHT,
            KeyEvent.KEYCODE_ALT_LEFT,
            KeyEvent.KEYCODE_ALT_RIGHT,
            KeyEvent.KEYCODE_META_LEFT,
            KeyEvent.KEYCODE_META_RIGHT,
            KeyEvent.KEYCODE_CAPS_LOCK,
            KeyEvent.KEYCODE_NUM_LOCK,
            KeyEvent.KEYCODE_SCROLL_LOCK,
            KeyEvent.KEYCODE_FUNCTION,
            -> return false
        }

        val meta = event.metaState
        val hasAltGr = (meta and KeyEvent.META_ALT_RIGHT_ON) != 0
        // Ctrl+key and left-Alt+key are key-code based (control byte / ESC prefix),
        // not layout dependent — let the encoder path handle them. AltGr is the
        // exception: it produces a composed character.
        if ((meta and KeyEvent.META_CTRL_ON) != 0 && !hasAltGr) return false
        if ((meta and KeyEvent.META_ALT_ON) != 0 && !hasAltGr) return false

        val unicodeChar = event.getUnicodeChar(meta)
        if (unicodeChar <= 0) return false

        val bridge = runtime.bridge() ?: return false

        // Build the modifier mask from the sticky toolbar state only. Shift is
        // already baked into the produced character by getUnicodeChar.
        val state = _state.value
        val mask = KeyModifiers.fromStickyStates(state.ctrlState, state.altState)

        // The unshifted codepoint is the base key with no modifiers applied:
        // recompute the character with SHIFT removed so the encoder can detect a
        // shift-only change (e.g. Shift+; ->:) and avoid a spurious Kitty shift.
        val unshiftedChar = event.getUnicodeChar(meta and KeyEvent.META_SHIFT_MASK.inv())
        val success = bridge.processKeyEvent(keyCode, mask.toByte(), 0, unicodeChar, unshiftedChar)
        if (success) {
            // Clear the one-shot (tapped) sticky modifier so it cannot persist
            // across the next keystroke (Haven TerminalViewModel.clearStickyModifiers,
            // #298). The encoder above already saw the active modifier for THIS
            // keystroke; consumption happens after the encode.
            consumeOneShotModifiers()
            Log.d(
                "TerminalViewModel",
                // Never log the character itself — hardware keyboard input
                // may contain passwords; the payload lands in the persisted
                // logcat dump (term_*.log).
                "handleLayoutAwareHardwareKey: keyCode=$keyCode mask=$mask",
            )
        }
        return success
    }

    fun toggleScrollMode() {
        _state.update { it.copy(scrollActive = !it.scrollActive) }
        // sync scrollActive to SessionEntry so the render thread
        // knows whether to auto-reset scroll on new output.
        runtime.setScrollActive(_state.value.scrollActive)
    }

    fun createSession() {
        val surface = currentSurface
        if (surface == null || !surface.isValid) {
            android.util.Log.e("TerminalViewModel", "createSession: surface null or invalid, currentSurface=$currentSurface")
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels <= 0 || surfaceHeightPixels <= 0) {
            android.util.Log.e("TerminalViewModel", "createSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels")
            return
        }

        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            val currentSurfaceNow = currentSurface
            if (currentSurfaceNow == null || !currentSurfaceNow.isValid) {
                android.util.Log.e("TerminalViewModel", "createSession: surface became invalid before launch")
                return@launch
            }
            try {
                val newId = runtime.createSession(currentSurfaceNow, surfaceWidthPixels, surfaceHeightPixels)
                if (newId > 0) {
                    _state.update { current ->
                        val sortedIds = (current.sessions.map { it.id } + newId).sorted()
                        val displayIndex = sortedIds.indexOf(newId) + 1
                        val sessions =
                            sortedIds.mapIndexed { index, id ->
                                SessionInfo(id = id, title = context.getString(R.string.session_number, index + 1))
                            }
                        current.copy(
                            sessionId = newId,
                            isRunning = true,
                            title = context.getString(R.string.session_number, displayIndex),
                            selection = SelectionState(),
                            sessions = sessions,
                            activeSessionId = newId,
                            selectionBg = runtime.selectionBgColor,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                } else {
                    android.util.Log.e("TerminalViewModel", "createSession: runtime returned invalid id=$newId")
                }
            } catch (exception: Exception) {
                android.util.Log.e("TerminalViewModel", "createSession failed", exception)
            }
        }
    }

    fun switchSession(id: Long) {
        val surface = currentSurface
        if (surface == null || !surface.isValid) {
            android.util.Log.e("TerminalViewModel", "switchSession: surface null or invalid, currentSurface=$currentSurface")
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels == 0 || surfaceHeightPixels == 0) {
            android.util.Log.e("TerminalViewModel", "switchSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels")
            return
        }

        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                runtime.switchSession(id, surface, surfaceWidthPixels, surfaceHeightPixels)
            } catch (exception: Exception) {
                android.util.Log.e("TerminalViewModel", "switchSession failed for id=$id", exception)
                return@launch
            }
            _state.update { current ->
                current.copy(
                    sessionId = id,
                    isRunning = true,
                    title =
                    runtime.state.value.title
                        .ifEmpty {
                            val sortedIds =
                                current.sessions
                                    .map { it.id }
                                    .sorted()
                            context.getString(R.string.session_number, sortedIds.indexOf(id) + 1)
                        },
                    activeSessionId = id,
                    selection = SelectionState(),
                    selectionBg = runtime.selectionBgColor,
                    selectionAccent = runtime.accentColor,
                )
            }
        }
    }

    fun closeSession() {
        closeSession(_state.value.activeSessionId)
    }

    fun closeSession(id: Long) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                runtime.closeSession(id)
            } catch (exception: Exception) {
                // closeSession must never escape to the main-thread uncaught
                // handler: BootGuard would treat it as a crash and kill the
                // process. The native side tolerates unknown/dead sessions.
                android.util.Log.e("TerminalViewModel", "closeSession failed for id=$id", exception)
                return@launch
            }
            withContext(Dispatchers.Main) {
                _state.update { current ->
                    val remaining = current.sessions.filter { it.id != id }
                    if (remaining.isEmpty()) {
                        current.copy(
                            isRunning = false,
                            sessions = emptyList(),
                            activeSessionId = 0L,
                            selection = SelectionState(),
                        )
                    } else {
                        val renumbered =
                            remaining.sortedBy { it.id }.mapIndexed { index, session ->
                                session.copy(title = context.getString(R.string.session_number, index + 1))
                            }
                        val newActive =
                            if (current.activeSessionId == id) {
                                remaining.last().id
                            } else {
                                current.activeSessionId
                            }
                        val newActiveIndex = renumbered.indexOfFirst { it.id == newActive }
                        current.copy(
                            sessions = renumbered,
                            activeSessionId = newActive,
                            sessionId = newActive,
                            title =
                            runtime.state.value.title
                                .ifEmpty { context.getString(R.string.session_number, newActiveIndex + 1) },
                            selection = SelectionState(),
                        )
                    }
                }
            }
        }
    }

    fun setSessionTitle(title: String) {
        _state.update { it.copy(title = title) }
    }
}
