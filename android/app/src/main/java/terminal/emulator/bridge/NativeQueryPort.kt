package terminal.emulator.bridge

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Native-backed [TerminalQueryPort]: every method maps 1:1 to a JNI
 * export (see `native/src/android/ffi.rs`, "JNI Exports:
 * TerminalQueryPort"). Used by [Bridge] for the live session; there is
 * no stub anymore (round-205).
 *
 * Contract for callers: null/0/empty means "no data" from the engine —
 * never fake data. Single-row/font queries are cheap; bulk queries
 * ([getTerminalText], [searchAllInScrollback]) are debounced by the UI.
 */
@Suppress("TooManyFunctions")
class NativeQueryPort(private val sessionIdProvider: () -> Long) : TerminalQueryPort {
    override fun getTitle(): String? = NativeBridge.getTitle(sessionIdProvider())

    override fun getActiveSessionTitle(): String = getTitle() ?: ""

    override fun setSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean?,
        mode: Byte,
        selectionBgArgb: Int,
    ) {
        // Selection is rendered through the render path (SelectionRange),
        // driven by TerminalSurface.consumeSelectionState. Forward to the
        // native render state so the GPU shader swaps the background
        // color of the selected cells (round-214).
        val active = hasSelection ?: true
        NativeBridge.setSelection(
            sessionIdProvider(),
            startRow,
            startCol,
            endRow,
            endCol,
            active,
            mode,
            selectionBgArgb,
        )
    }

    override fun expandAndSetSelection(row: Int, col: Int, mode: Byte): Pair<Pair<Int, Int>, Pair<Int, Int>>? {
        // Smart word/URL boundary detection on the visible line: fetch the
        // line text from native, expand bounds in pure Kotlin (testable),
        // then apply the expanded range through setSelection.
        val line = scrollbackLine(row)
        if (line == null) {
            // Blank line: caller falls back to single-cell selection +
            // paste menu (long-press on whitespace). Not an error.
            return null
        }
        val (startCol, endCol) = SelectionExpander.expandBounds(line, col)
        setSelection(row, startCol, row, endCol, true, mode)
        return (row to startCol) to (row to endCol)
    }

    override fun clearSearchHighlights() {
        NativeBridge.clearSearchHighlights(sessionIdProvider())
    }

    override fun setSearchHighlights(data: ByteArray) {
        NativeBridge.setSearchHighlights(sessionIdProvider(), data)
    }

    override fun scrollbackLine(row: Int): String? = NativeBridge.scrollbackLine(sessionIdProvider(), row)

    override fun scrollbackLength(): Int = NativeBridge.scrollbackLength(sessionIdProvider())

    override fun isCellEmpty(row: Int, col: Int): Boolean = NativeBridge.isCellEmpty(sessionIdProvider(), row, col)

    override fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>? = NativeBridge.searchAllInScrollback(sessionIdProvider(), query, caseSensitive, fuzzyMatch)
        ?.let { parseSearchMatches(it) }

    override fun setScrollOffset(offset: Int) {
        // Round-205: the native side applies the delta on the VT thread
        // via scroll_viewport, so the next CellData push carries the
        // scrolled view. Previously a no-op — scrollback browsing did
        // nothing.
        NativeBridge.setScrollOffset(sessionIdProvider(), offset)
    }

    override fun getTerminalText(): String? = NativeBridge.getTerminalText(sessionIdProvider())

    override fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int, rectangle: Boolean): String? =
        NativeBridge.selectionText(sessionIdProvider(), startRow, startCol, endRow, endCol, rectangle)

    override fun hyperlinkAt(row: Int, col: Int): String? = NativeBridge.hyperlinkAt(sessionIdProvider(), row, col)

    override fun listFontFamilies(): List<String>? = NativeBridge.listFontFamilies()?.toList()

    override fun getDefaultFontName(): String = NativeBridge.getDefaultFontName() ?: "monospace"

    override fun getFontInfo(): String? = NativeBridge.getFontInfo()
}

@Serializable
internal data class SearchMatchDto(
    val row: Int = 0,
    val start_col: Int = 0,
    val end_col: Int = 0,
)

/**
 * Parses the JSON array of `{"row":int,"start_col":int,"end_col":int}`
 * produced by the native `searchAllInScrollback` export. Returns an empty
 * list on malformed input (never throws) so search degrades to
 * "no results" instead of crashing the UI.
 *
 * Results with impossible ranges (negative, end <= start) are dropped —
 * a missing field would otherwise silently produce a bogus row=0 match
 * that highlights the wrong line.
 */
private val searchJson = Json { ignoreUnknownKeys = true }

internal fun parseSearchMatches(json: String): List<Triple<Int, Int, Int>> = try {
    searchJson
        .decodeFromString<List<SearchMatchDto>>(json)
        .filter { it.row >= 0 && it.start_col >= 0 && it.end_col > it.start_col }
        .map { Triple(it.row, it.start_col, it.end_col) }
} catch (_: Exception) {
    emptyList()
}
