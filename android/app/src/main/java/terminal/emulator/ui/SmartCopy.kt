package terminal.emulator.ui

/**
 * Smart copy processing (round-225) — mirrors Haven SelectionToolbar
 * smartCopy (:357-405) + SmartTerminalClipboard (:407-430):
 *
 *  1. TUI border stripping: when vertical box-drawing border characters
 *     appear consistently at the same columns across ≥60% of the selected
 *     non-blank lines, extract only the panel content bounded by the
 *     nearest borders around the selection start column.
 *  2. Wrapped-URL rebuild: a multi-row selection whose trimmed rows rejoin
 *     into a single complete URL is returned without newlines.
 *  3. Verbatim fallback: anything else returns the caller's raw text.
 *
 * Pure Kotlin so every rule is unit-testable without the native engine.
 */
object SmartCopy {

    /** Vertical box-drawing border characters (Haven smartCopy:357). */
    internal val VERTICAL_BORDERS =
        setOf('│', '┃', '║', '|', '┆', '┇', '┊', '┋')

    internal fun isVerticalBorder(ch: Char): Boolean = ch in VERTICAL_BORDERS

    /**
     * Column indices where a vertical border appears in at least 60% of the
     * non-blank lines (minimum 2 occurrences). Empty when the selection has
     * fewer than 2 non-blank lines or no column reaches the threshold.
     * Mirrors Haven findConsistentBorderColumns (:299-323).
     */
    internal fun findConsistentBorderColumns(lines: List<String>): Set<Int> {
        if (lines.size < 2) return emptySet()
        val nonEmptyLines = lines.count { it.isNotBlank() }
        if (nonEmptyLines < 2) return emptySet()
        val maxLen = lines.maxOf { it.length }
        if (maxLen == 0) return emptySet()
        val borderCounts = IntArray(maxLen)
        for (line in lines) {
            for ((col, ch) in line.withIndex()) {
                if (isVerticalBorder(ch)) borderCounts[col]++
            }
        }
        val threshold = (nonEmptyLines * 0.6).toInt().coerceAtLeast(2)
        return borderCounts.indices.filter { borderCounts[it] >= threshold }.toSet()
    }

    /**
     * Content of the panel containing [startCol], bounded by the nearest
     * consistent border left of it and the nearest border right of it,
     * trimmed per row and joined with '\n' (Haven:325-345).
     */
    internal fun extractPanelContent(
        lines: List<String>,
        borderCols: Set<Int>,
        startCol: Int,
    ): String {
        val sortedBorders = borderCols.sorted()
        val leftBorder = sortedBorders.lastOrNull { it < startCol } ?: -1
        val rightBorder =
            sortedBorders.firstOrNull { it > startCol } ?: (lines.maxOfOrNull { it.length } ?: 0)
        return lines
            .map { line ->
                val start = (leftBorder + 1).coerceAtLeast(0)
                val end = rightBorder.coerceAtMost(line.length)
                if (start < end) line.substring(start, end).trim() else ""
            }
            .joinToString("\n")
            .trimEnd()
    }

    /**
     * Reconstruct a URL from a multi-row selection by stripping each row's
     * leading/trailing whitespace and joining without newlines. Returns the
     * rebuilt URL only when it forms a single complete URL token — otherwise
     * null so the caller keeps verbatim text.
     */
    internal fun rebuildWrappedUrl(
        lines: List<String>,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
    ): String? {
        val sb = StringBuilder()
        for (r in startRow..endRow) {
            val line = lines.getOrNull(r) ?: return null
            val from = if (r == startRow) startCol.coerceIn(0, line.length) else 0
            val to = if (r == endRow) (endCol + 1).coerceIn(0, line.length) else line.length
            if (from < to) sb.append(line.substring(from, to).trim())
        }
        val joined = sb.toString()
        return if (looksLikeFullUrl(joined)) joined else null
    }

    /**
     * Smart copy over the given selection lines: border-strip first, then
     * wrapped-URL rebuild for multi-row selections, else [verbatim].
     * Mirrors Haven smartCopy's precedence (border path bypasses soft-wrap
     * rejoin; URL path only replaces when it forms a clean token).
     */
    internal fun smartCopyText(
        lines: List<String>,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        verbatim: String?,
    ): String {
        val fullTexts = lines
        val borderCols = findConsistentBorderColumns(fullTexts)
        if (borderCols.isNotEmpty()) {
            return extractPanelContent(fullTexts, borderCols, startCol)
        }
        if (endRow > startRow) {
            rebuildWrappedUrl(fullTexts, startRow, startCol, endRow, endCol)
                ?.let { return it }
        }
        return verbatim ?: ""
    }

    /**
     * Whether a whitespace-free string looks like a *complete* URL token
     * (scheme- or `www.`-prefixed, a dotted host, optional path/query).
     */
    private val URL_TOKEN_RE =
        Regex(
            "^(?:https?://|www\\.)[\\w-]+(?:\\.[\\w-]+)+(?:[/:?#@!\$&'()*+,;=._~%\\[\\]-]\\S*)?$",
            RegexOption.IGNORE_CASE,
        )

    internal fun looksLikeFullUrl(s: String): Boolean = URL_TOKEN_RE.matches(s)
}
