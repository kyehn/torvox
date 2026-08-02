package terminal.emulator.bridge

/**
 * Kotlin-side query port for terminal content that is not yet available
 * through the native render path.
 *
 * ADR-0007 (surface integration) is not wired yet: the native query path
 * does not exist, so scrollback/search/selection queries would return
 * nothing. Instead of sprinkling `?: ""` / try-catch defense across the
 * UI, every caller depends on this interface. Two implementations:
 *
 * - [StubQueryPort] — returns conservative defaults ("no data"), used
 *   until the native path lands.
 * - (future) `NativeQueryPort` — backed by JNI, swapped in when
 *   ADR-0007 is implemented; no UI file changes compile-time.
 *
 * Contract for callers:
 * - `scrollbackLine`/`scrollbackLength`/`searchAllInScrollback` return
 *   null/0/empty-list: "no data". Treat as unavailable, not "empty
 *   content" — faking data would corrupt selections and search results
 *   once the native path lands.
 * - `isCellEmpty` returns true: long-press opens the paste popup (the
 *   only long-press action usable without native data).
 */
// The query surface is intentionally wide: it mirrors the native exports
// one-to-one so the seam can be swapped without UI churn.
@Suppress("TooManyFunctions")
interface TerminalQueryPort {
    fun getTitle(): String?
    fun getActiveSessionTitle(): String = getTitle() ?: ""

    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean? = null, mode: Byte = 0)
    fun expandAndSetSelection(row: Int, col: Int, mode: Byte = 0): Pair<Pair<Int, Int>, Pair<Int, Int>>?

    fun clearSearchHighlights()
    fun setSearchHighlights(data: ByteArray)
    fun scrollbackLine(row: Int): String?
    fun scrollbackLength(): Int
    fun isCellEmpty(row: Int, col: Int): Boolean
    fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>?
    fun setScrollOffset(offset: Int)

    fun getTerminalText(): String?
    fun listFontFamilies(): List<String>?
    fun getDefaultFontName(): String
    fun getFontInfo(): Bridge.FontInfo?
}

/** Query port returning conservative defaults until ADR-0007 lands. */
class StubQueryPort : TerminalQueryPort {
    override fun getTitle(): String? = null

    override fun setSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean?,
        mode: Byte,
    ) = Unit

    override fun expandAndSetSelection(row: Int, col: Int, mode: Byte): Pair<Pair<Int, Int>, Pair<Int, Int>>? = null

    override fun clearSearchHighlights() = Unit
    override fun setSearchHighlights(data: ByteArray) = Unit
    override fun scrollbackLine(row: Int): String? = null
    override fun scrollbackLength(): Int = 0
    override fun isCellEmpty(row: Int, col: Int): Boolean = true
    override fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>? = null
    override fun setScrollOffset(offset: Int) = Unit

    override fun getTerminalText(): String? = null
    override fun listFontFamilies(): List<String>? = null
    override fun getDefaultFontName(): String = "monospace"
    override fun getFontInfo(): Bridge.FontInfo? = null
}
