package terminal.emulator.bridge

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Native-backed [TerminalQueryPort]: every method maps 1:1 to a JNI
 * export (see `native/src/android/ffi.rs`, "JNI Exports:
 * TerminalQueryPort"). Used by [Bridge] for the live session; there is
 * no stub anymore.
 *
 * Contract for callers: null/0/empty means "no data" from the engine —
 * never fake data. Single-row/font queries are cheap; bulk queries
 * ([getTerminalText], [searchAllInScrollback]) are debounced by the UI.
 */
class NativeQueryPort(private val sessionIdProvider: () -> Long) : TerminalQueryPort {
    override fun getTitle(): String? = NativeBridge.getTitle(sessionIdProvider())

    override fun getActiveSessionTitle(): String = getTitle() ?: ""

    override fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?) {
        // Selection lives in the terminal (tracked refs, installed via
        // NativeBridge.setSelection): the VT thread bakes the inverse
        // video into CellData, so no view-side cell bookkeeping is needed.
        val active = hasSelection ?: true
        NativeBridge.setSelection(
            sessionIdProvider(),
            startRow,
            startCol,
            endRow,
            endCol,
            active,
        )
    }

    override fun clearSearchHighlights() {
        NativeBridge.clearSearchHighlights(sessionIdProvider())
    }

    override fun setSearchHighlights(data: ByteArray) {
        NativeBridge.setSearchHighlights(sessionIdProvider(), data)
    }

    override fun scrollbackLine(row: Int): String? = NativeBridge.scrollbackLine(sessionIdProvider(), row)

    override fun scrollbackLength(): Int = NativeBridge.scrollbackLength(sessionIdProvider())

    override fun cursorViewportPacked(): Long = NativeBridge.getCursorViewportPacked(sessionIdProvider())

    override fun isCellEmpty(row: Int, col: Int): Boolean = NativeBridge.isCellEmpty(sessionIdProvider(), row, col)

    override fun searchAllInScrollback(query: String, caseSensitive: Boolean): List<Triple<Int, Int, Int>>? =
        NativeBridge.searchAllInScrollback(
            sessionIdProvider(),
            query,
            caseSensitive,
        )
            ?.let { parseSearchMatches(it) }

    override fun setScrollOffset(offset: Int) {
        // the native side applies the delta on the VT thread
        // via scroll_viewport, so the next CellData push carries the
        // scrolled view. Previously a no-op — scrollback browsing did
        // nothing.
        NativeBridge.setScrollOffset(sessionIdProvider(), offset)
    }

    override fun setScrollYPx(offsetPx: Float) {
        NativeBridge.setScrollYPx(sessionIdProvider(), offsetPx)
    }

    override fun getTerminalText(): String? = NativeBridge.getTerminalText(sessionIdProvider())

    override fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String? =
        NativeBridge.selectionText(
            sessionIdProvider(),
            startRow,
            startCol,
            endRow,
            endCol,
        )

    override fun hyperlinkAt(row: Int, col: Int): String? = NativeBridge.hyperlinkAt(sessionIdProvider(), row, col)

    override fun selectWordAt(row: Int, col: Int): IntArray? = NativeBridge.selectWordAt(sessionIdProvider(), row, col)

    override fun selectLineAt(row: Int, col: Int): IntArray? = NativeBridge.selectLineAt(sessionIdProvider(), row, col)

    override fun selectAll(): IntArray? = NativeBridge.selectAll(sessionIdProvider())

    override fun listFontFamilies(): List<String>? = NativeBridge.listFontFamilies()?.toList()

    override fun getDefaultFontName(): String = NativeBridge.getDefaultFontName() ?: ""

    override fun getFontInfo(): String? = NativeBridge.getFontInfo()
}

@Serializable
internal data class SearchMatchDto(val row: Int = 0, val start_col: Int = 0, val end_col: Int = 0)

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
