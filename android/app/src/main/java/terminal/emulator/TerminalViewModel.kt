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
import terminal.emulator.bridge.NativeBridge
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
    // floating menu closes while the selection highlight stays (round-214).
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
}

data class HandleDragResult(
    val startRow: Int,
    val startCol: Int,
    val endRow: Int,
    val endCol: Int,
)

data class PastePopupRequest(
    val row: Int,
    val col: Int,
)

/** State for multi-line paste confirmation dialog. */
data class PasteConfirmationState(
    val visible: Boolean = false,
    val text: String = "",
    val lineCount: Int = 0,
    val charCount: Int = 0,
)

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
    val pastePopupRequest: PastePopupRequest? = null,
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
    private val fontManager = FontManager()
    private val userThemeStore = UserThemeStore(context)

    // Hot StateFlow mirror of the DataStore (cold flow + recomposition would
    // resubscribe per frame and miss updates; stateIn keeps one collector).
    private val _userThemes = kotlinx.coroutines.flow.MutableStateFlow<List<terminal.emulator.ui.theme.TerminalTheme>>(emptyList())
    val userThemes: kotlinx.coroutines.flow.StateFlow<List<terminal.emulator.ui.theme.TerminalTheme>> = _userThemes.asStateFlow()

    // ── Font forwards (implementation in FontManager) ─────────────────────

    fun setFontSize(size: Float) = fontManager.setFontSize(size)

    fun setFontFamily(family: String) = fontManager.setFontFamily(family)

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

    fun consumePastePopupRequest(): PastePopupRequest? = selectionManager.consumePastePopupRequest()

    fun shareSelection() = selectionManager.shareSelection()

    /**
     * Export full terminal text (scrollback + visible) to a SAF URI.
     * Called from the SelectionActions toolbar via ActivityResultContracts.CreateDocument.
     */
    fun exportTerminalOutput(uri: Uri) {
        viewModelScope.launch {
            val text = withContext(Dispatchers.IO) {
                runtime.bridge()?.getTerminalText() ?: ""
            }
            if (text.isEmpty()) {
                android.widget.Toast.makeText(context, "Terminal output is empty", android.widget.Toast.LENGTH_SHORT).show()
                return@launch
            }
            try {
                withContext(Dispatchers.IO) {
                    context.contentResolver.openOutputStream(uri)?.use { out ->
                        out.write(text.toByteArray(Charsets.UTF_8))
                    }
                }
                android.widget.Toast.makeText(context, "Terminal output exported", android.widget.Toast.LENGTH_SHORT).show()
            } catch (e: java.io.IOException) {
                LogUtil.e("ViewModel", "Export failed", e)
                android.widget.Toast.makeText(context, "Export failed: ${e.message}", android.widget.Toast.LENGTH_SHORT).show()
            }
        }
    }

    fun selectAll(scrollOffset: Int = 0) = selectionManager.selectAll(scrollOffset)

    fun moveSelectionAnchor(deltaCol: Int) = selectionManager.moveSelectionAnchor(deltaCol)

    fun pasteFromClipboard(): Int = selectionManager.pasteFromClipboard()

    fun confirmPaste() {
        val text = _pasteConfirmation.value.text
        _pasteConfirmation.value = PasteConfirmationState()
        selectionManager.executePaste(text)
    }

    fun cancelPaste() {
        _pasteConfirmation.value = PasteConfirmationState()
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
     * (kotlin-architecture-round2 K5). Inner class: accesses _state,
     * runtime and clipboardPaster via the outer view model.
     */
    inner class SelectionManager {
        fun startSelection(
            row: Int,
            col: Int,
            touchClass: TouchClass = TouchClass.Unknown,
        ) {
            // A text-selection long-press supersedes any pending paste chip;
            // drop it so it cannot reappear once the selection is cleared
            // (round-105).
            consumePastePopupRequest()
            val anchor = SelectionAnchor(row, col)
            // CAS (round-98): selection is touched from the main thread but
            // _state is also written by IO coroutines; a plain RMW could lose
            // one of their updates. mode is read inside the lambda. (The old
            // EmptyArea override to Char mode was dead — no caller passes
            // EmptyArea; round-99.)
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
            // CAS with the active-check inside the lambda (round-98): the
            // selection read and the write are atomic against concurrent _state
            // updates. Paste-only selections (empty-cell/whitespace long-press)
            // are immutable — a finger micro-move during the long-press must
            // not drift the single cell (round-214).
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
            // CAS loop (round-101): compareAndSet retries while the state is
            // still ours and aborts as soon as a concurrent write lands — the
            // native boundary sync below runs only for a genuinely committed
            // snapshot (no side-effect flag that could leak across retries).
            // Note (round-100): extractSelectedText itself reads the bridge and
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
            _state.update { it.copy(selection = it.selection.copy(mode = mode)) }
        }

        fun copySelectionToClipboard() {
            val rawText = _state.value.selection.selectedText
            if (rawText.isEmpty()) return
            // Smart processing (round-225, Haven smartCopy:357-405) applies
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
         * Round-225: route the copy through smart processing (TUI border
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
            // Drop any pending paste chip: with the selection cleared it would
            // reappear over the terminal (round-105).
            consumePastePopupRequest()
            syncSelectionToNative()
        }

        fun showPastePopup(
            row: Int,
            col: Int,
        ) {
            // Round-214: an empty-cell long-press now creates a single-cell
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
                    pastePopupRequest = null,
                )
            }
            syncSelectionToNative()
        }

        fun consumePastePopupRequest(): PastePopupRequest? {
            // Explicit CAS loop (round-102): update() would re-run its lambda on
            // contention and leak a stale captured result to a second consumer;
            // compareAndSet returns exactly one winner per request.
            while (true) {
                val state = _state.value
                val req = state.pastePopupRequest ?: return null
                if (_state.compareAndSet(state, state.copy(pastePopupRequest = null))) return req
            }
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
            // Round-225: share the smart-copied text (border strip / URL rebuild).
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
            // Close the floating menu after the action (round-214).
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

        @Suppress("CyclomaticComplexMethod")
        private fun extractSelectedText(selection: SelectionState): String {
            val start = selection.start ?: return ""
            val end = selection.end ?: return ""
            val bridge = runtime.bridge() ?: return ""
            // Selection rows are stored in grid coordinates (0 = top of
            // scrollback), matching the Ghostty formatter's grid rows.
            //
            // Wrap-aware extraction (termux TerminalBuffer.getSelectedText
            // semantics, round-218): soft-wrapped rows are joined without
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

        private fun isWideChar(ch: Char): Boolean {
            val type = Character.getType(ch)
            return type == Character.OTHER_SYMBOL.toInt() ||
                type == Character.LETTER_NUMBER.toInt() ||
                type == Character.ENCLOSING_MARK.toInt() ||
                ch.code in 0x1100..0x115F ||
                ch.code in 0x2E80..0x9FFF ||
                ch.code in 0xA000..0xA4CF ||
                ch.code in 0xAC00..0xD7AF ||
                ch.code in 0xF900..0xFAFF ||
                ch.code in 0xFE30..0xFE6F ||
                ch.code in 0xFF01..0xFF60 ||
                ch.code in 0xFFE0..0xFFE6 ||
                ch.code in 0x20000..0x2FA1F ||
                ch.code in 0x30000..0x3134F
        }

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

        /**
         * Paste clipboard content. For multi-line or long content (>1 line or
         * >500 chars), shows a confirmation dialog first.
         * Reference: research-gnome-console.md §4 paste confirmation.
         */
        fun pasteFromClipboard(): Int {
            val text = clipboardAccess.clipboardText() ?: return 0
            val lines = text.lines()
            if (lines.size > 1 || text.length > 500) {
                _pasteConfirmation.update {
                    PasteConfirmationState(
                        visible = true,
                        text = text,
                        lineCount = lines.size,
                        charCount = text.length,
                    )
                }
                _state.update { it.copy(selection = it.selection.copy(menuDismissed = true)) }
                return 0
            }
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
     * Extracted from TerminalViewModel (kotlin-architecture-round3 R4).
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
                        bridge?.getFontInfo() ?: "Font: ${_defaultFontName.value}\n(CJK fallback info available after session starts)"
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
                    _fontInfo.value = bridge.getFontInfo() ?: "No font loaded"
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
                    val fontInfo = bridge?.getFontInfo() ?: "No font loaded"
                    _defaultFontName.value = fontName
                    _fontInfo.value = fontInfo
                    android.util.Log.d("Font", "Font applied: $fontName")
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(context, "Font applied: $fontName", android.widget.Toast.LENGTH_SHORT)
                            .show()
                    }
                } catch (exception: Exception) {
                    android.util.Log.e("Font", "setFontFamily failed for $family", exception)
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(
                                context,
                                "Font apply failed: ${exception.message}",
                                android.widget.Toast.LENGTH_SHORT,
                            ).show()
                    }
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
                                .makeText(context, "Failed to read font file", android.widget.Toast.LENGTH_SHORT)
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
                                .makeText(context, "Font installed: $familyName", android.widget.Toast.LENGTH_SHORT)
                                .show()
                        }
                    } else {
                        android.util.Log.e("Font", "Font load failed: null family from ${destFile.absolutePath}")
                        kotlinx.coroutines.withContext(Dispatchers.Main) {
                            android.widget.Toast
                                .makeText(context, "Font not supported or corrupted", android.widget.Toast.LENGTH_SHORT)
                                .show()
                        }
                    }
                } catch (exception: Exception) {
                    android.util.Log.e("TerminalViewModel", "installFontFile failed", exception)
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(
                                context,
                                "Font installation failed: ${exception.message}",
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

    private val _pasteConfirmation = MutableStateFlow(PasteConfirmationState())
    val pasteConfirmation: StateFlow<PasteConfirmationState> = _pasteConfirmation.asStateFlow()

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
                SettingsRepository.SettingsState(),
            )

    private val _availableFonts = MutableStateFlow<List<String>>(emptyList())
    val availableFonts: StateFlow<List<String>> = _availableFonts.asStateFlow()

    private val _defaultFontName = MutableStateFlow("")
    val defaultFontName: StateFlow<String> = _defaultFontName.asStateFlow()

    private val _fontInfo = MutableStateFlow("Active: monospace\n(CJK fallback info available after session starts)")
    val fontInfo: StateFlow<String> = _fontInfo.asStateFlow()

    init {
        viewModelScope.launch {
            runtime.state.collect { runtimeState ->
                val sortedIds = runtimeState.sessionIds.sorted()
                val sessions =
                    sortedIds.mapIndexed { index, id ->
                        SessionInfo(id = id, title = "Session ${index + 1}")
                    }
                val active = runtimeState.activeSessionId
                if (active != 0L) {
                    val displayIndex = sortedIds.indexOf(active) + 1
                    val title =
                        if (runtimeState.title.isNotEmpty()) {
                            runtimeState.title
                        } else {
                            "Session $displayIndex"
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
                            _fontInfo.value = bridge?.getFontInfo() ?: "No font loaded"
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
            // the sliders (round-203, emulator-verified).
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

    fun setTouchBehavior(behavior: String) {
        viewModelScope.launch {
            settingsRepository.setTouchBehavior(behavior)
        }
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

    fun runBootstrap() {
        // CAS so a rapid double-tap of the Install button cannot start two
        // concurrent installs (round-109).
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
                // (even if cleared) field wins over the stored URL (round-109).
                val url =
                    if (bootstrapUrlEdited) {
                        bootstrapUrlDebounce.value
                    } else {
                        settingsRepository.bootstrapUrl.first()
                    }
                val result = orchestrator.ensureBootstrap(url)
                _bootstrapResult.value = result.getOrNull() ?: "Error: ${result.exceptionOrNull()?.javaClass?.simpleName}"
            } catch (exception: Exception) {
                _bootstrapResult.value = "Error: ${exception.javaClass.simpleName}"
            } finally {
                _bootstrapRunning.value = false
            }
        }
    }

    /**
     * Offline bootstrap install from a SAF URI.  The user picks a .zip file
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
                    val messages = mutableListOf("Bootstrap installed from file")
                    if (secondResult.errors.isNotEmpty()) {
                        messages.add("${secondResult.errors.size} postinst scripts had errors")
                    }
                    _bootstrapResult.value = messages.joinToString("; ")
                } else {
                    _bootstrapResult.value = "Error: ${result.exceptionOrNull()?.javaClass?.simpleName}"
                }
                cacheFile.delete()
            } catch (exception: Exception) {
                LogUtil.e("ViewModel", "Offline install failed", exception)
                _bootstrapResult.value = "Error: ${exception.javaClass.simpleName}"
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

    fun setUseSemanticSelection(enabled: Boolean) {
        viewModelScope.launch {
            settingsRepository.setUseSemanticSelection(enabled)
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
                try {
                    val dst = java.io.File(context.filesDir, "terminal_background")
                    context.contentResolver.openInputStream(android.net.Uri.parse(path))?.use { input ->
                        java.io.FileOutputStream(dst).use { output ->
                            val buf = ByteArray(64 * 1024)
                            while (true) {
                                val n = input.read(buf)
                                if (n <= 0) break
                                output.write(buf, 0, n)
                            }
                        }
                    }
                    effectivePath = dst.absolutePath
                    LogUtil.d(TAG, "background image copied to private storage: $effectivePath")
                } catch (e: Throwable) {
                    LogUtil.e(TAG, "background image private copy failed, keeping original path", e)
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
            // Wait up to 15s for a bridge (emulator-verified, round-203:
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
                    val uri = android.net.Uri.parse(path)
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
                            // read (round-202, emulator-verified).
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
                        // Byte order note (round-211, emulator-verified):
                        // Coil 3 with .allowHardware(false) returns a bitmap
                        // whose copyPixelsToBuffer() output is already RGBA
                        // byte order on this pipeline. The earlier BGRA→RGBA
                        // swap (round-210) double-swapped and rendered the
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
     * render. Shared by the three cursor setters (R10: round-3
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

    fun setDayThemeName(name: String) = applyThemeSettings { settingsRepository.setDayThemeName(name) }

    fun setNightThemeName(name: String) = applyThemeSettings { settingsRepository.setNightThemeName(name) }

    fun setThemeMode(mode: String) = applyThemeSettings { settingsRepository.setThemeMode(mode) }

    fun setAppThemeMode(mode: String) = applyThemeSettings { settingsRepository.setAppThemeMode(mode) }

    /**
     * Persist a theme setting then re-apply the whole theme to the bridge.
     * Shared by the five theme setters (R10: round-3 architecture).
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
    // (round-110).
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
        // shift-only change (e.g. Shift+; -> :) and avoid a spurious Kitty shift.
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
                                SessionInfo(id = id, title = "Session ${index + 1}")
                            }
                        current.copy(
                            sessionId = newId,
                            isRunning = true,
                            title = "Session $displayIndex",
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
                            "Session ${sortedIds.indexOf(id) + 1}"
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
                                session.copy(title = "Session ${index + 1}")
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
                                .ifEmpty { "Session ${newActiveIndex + 1}" },
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
