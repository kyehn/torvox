package terminal.emulator.bridge

/**
 * Kotlin-side query port for terminal content.
 *
 * ADR-0007 (surface integration) is implemented: the
 * native query path exists and is backed by JNI. Instead of sprinkling
 * `?: ""` / try-catch defense across the UI, every caller depends on
 * this interface. The live implementation is [NativeQueryPort] (JNI);
 * [StubQueryPort] was removed as dead code.
 *
 * Contract for callers:
 * - `scrollbackLine`/`scrollbackLength`/`searchAllInScrollback` return
 *   null/0/empty-list: "no data". Treat as unavailable, not "empty
 *   content" — faking data would corrupt selections and search results.
 * - `isCellEmpty` returns true: long-press opens the paste popup (the
 *   only long-press action usable without native data).
 */
// The query surface is intentionally wide: it mirrors the native exports
// one-to-one so the seam can be swapped without UI churn.
@Suppress("TooManyFunctions")
interface TerminalQueryPort {
    fun getTitle(): String?
    fun getActiveSessionTitle(): String = getTitle() ?: ""

    fun setSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean? = null,
        mode: Byte = 0,
        selectionBgArgb: Int = 0xFF45475A.toInt(),
    )
    fun expandAndSetSelection(row: Int, col: Int, mode: Byte = 0): Pair<Pair<Int, Int>, Pair<Int, Int>>?

    fun clearSearchHighlights()
    fun setSearchHighlights(data: ByteArray)
    fun scrollbackLine(row: Int): String?
    fun scrollbackLength(): Int

    /** Cursor viewport position packed `(y << 32) | x`, or -1 when hidden. */
    fun cursorViewportPacked(): Long
    fun isCellEmpty(row: Int, col: Int): Boolean
    fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>?
    fun setScrollOffset(offset: Int)

    /** Viewport Y pixel remainder for per-pixel smooth scrolling (positive = content down). */
    fun setScrollYPx(offsetPx: Float)

    fun getTerminalText(): String?
    fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int, rectangle: Boolean): String?
    fun hyperlinkAt(row: Int, col: Int): String?
    fun listFontFamilies(): List<String>?
    fun getDefaultFontName(): String
    fun getFontInfo(): String?
}
