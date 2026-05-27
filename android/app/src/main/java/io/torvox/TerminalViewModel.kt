package io.torvox

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject

enum class SelectionMode {
    Char,
    Word,
    Line,
    Block,
}

data class SelectionAnchor(
    val row: Int,
    val col: Int,
)

data class SelectionState(
    val active: Boolean = false,
    val start: SelectionAnchor? = null,
    val end: SelectionAnchor? = null,
    val mode: SelectionMode = SelectionMode.Char,
    val selectedText: String = "",
)

data class TerminalState(
    val sessionId: Long = 0L,
    val isRunning: Boolean = false,
    val title: String = "Torvox",
    val selection: SelectionState = SelectionState(),
)

@HiltViewModel
class TerminalViewModel
    @Inject
    constructor(
        @ApplicationContext private val context: Context,
    ) : ViewModel() {
        private val _state = MutableStateFlow(TerminalState())
        val state: StateFlow<TerminalState> = _state.asStateFlow()

        fun startSelection(
            row: Int,
            col: Int,
        ) {
            val anchor = SelectionAnchor(row, col)
            _state.value =
                _state.value.copy(
                    selection =
                        SelectionState(
                            active = true,
                            start = anchor,
                            end = anchor,
                            mode = _state.value.selection.mode,
                        ),
                )
        }

        fun updateSelection(
            row: Int,
            col: Int,
        ) {
            val current = _state.value.selection
            if (!current.active) return
            _state.value =
                _state.value.copy(
                    selection = current.copy(end = SelectionAnchor(row, col)),
                )
        }

        fun endSelection() {
            val current = _state.value.selection
            if (!current.active || current.start == null || current.end == null) return
            val text = extractSelectedText(current)
            _state.value =
                _state.value.copy(
                    selection = current.copy(active = false, selectedText = text),
                )
        }

        fun setSelectionMode(mode: SelectionMode) {
            _state.value =
                _state.value.copy(
                    selection = _state.value.selection.copy(mode = mode),
                )
        }

        fun copySelectionToClipboard() {
            val text = _state.value.selection.selectedText
            if (text.isEmpty()) return
            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clip = ClipData.newPlainText("terminal selection", text)
            clipboard.setPrimaryClip(clip)
        }

        fun openUrl(url: String) {
            try {
                val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                context.startActivity(intent)
            } catch (_: Exception) {
            }
        }

        fun clearSelection() {
            _state.value =
                _state.value.copy(
                    selection = SelectionState(),
                )
        }

        private fun extractSelectedText(selection: SelectionState): String {
            val start = selection.start ?: return ""
            val end = selection.end ?: return ""
            val (lo, hi) =
                if (start.row < end.row || (start.row == end.row && start.col <= end.col)) {
                    start to end
                } else {
                    end to start
                }
            return when (selection.mode) {
                SelectionMode.Char, SelectionMode.Word -> {
                    if (lo.row == hi.row) {
                        "selection[${lo.row}:${lo.col}-${hi.col}]"
                    } else {
                        "selection[${lo.row}:${lo.col}-${hi.row}:${hi.col}]"
                    }
                }

                SelectionMode.Line -> {
                    "selection[line:${lo.row}-${hi.row}]"
                }

                SelectionMode.Block -> {
                    "selection[block:${lo.row}:${lo.col}-${hi.row}:${hi.col}]"
                }
            }
        }
    }
