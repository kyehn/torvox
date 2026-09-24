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
interface TerminalQueryPort {
    fun getTitle(): String?
    fun getActiveSessionTitle(): String = getTitle() ?: ""

    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean? = null)

    fun clearSearchHighlights()
    fun setSearchHighlights(data: ByteArray)
    fun scrollbackLine(row: Int): String?
    fun scrollbackLength(): Int

    /** Cursor viewport position packed `(y << 32) | x`, or -1 when hidden. */
    fun cursorViewportPacked(): Long
    fun isCellEmpty(row: Int, col: Int): Boolean
    fun searchAllInScrollback(query: String, caseSensitive: Boolean): List<Triple<Int, Int, Int>>?
    fun setScrollOffset(offset: Int)

    /** Viewport Y pixel remainder for per-pixel smooth scrolling (positive = content down). */
    fun setScrollYPx(offsetPx: Float)

    fun getTerminalText(): String?
    fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String?

    // 上游选择派生（native select_word/select_line/select_all）：native 侧
    // 已把选区安装为终端状态，回传有序界限 [startRow, startCol, endRow,
    // endCol]（绝对网格坐标）。null = 无可选内容，按“无数据”处理，勿伪造。
    fun selectWordAt(row: Int, col: Int): IntArray?
    fun selectLineAt(row: Int, col: Int): IntArray?
    fun selectAll(): IntArray?

    fun hyperlinkAt(row: Int, col: Int): String?
    fun listFontFamilies(): List<String>?
    fun getDefaultFontName(): String
    fun getFontInfo(): String?
}
