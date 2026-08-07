package terminal.emulator.bridge

/**
 * Smart selection boundary expansion: given a line of terminal text and a
 * tapped column, expands to a whole word / URL / path run.
 *
 * Pure Kotlin so the boundary rules are unit-testable without the native
 * engine (round-214). Used by [NativeQueryPort.expandAndSetSelection].
 *
 * References:
 *  - termux-app TextSelectionCursorController.setInitialTextSelectionPosition
 *    (:93-108): whitespace stops expansion, non-blank expands to the token.
 *  - Haven SelectionToolbar.expandSelectionToWord (:40-90) + expandAcrossUrlWrap
 *    (:120-214): non-whitespace token + wrapped-URL continuation across rows —
 *    the multi-row URL case is NOT handled here yet (single-line only).
 *  - termlib SelectionManager.adjustSelectionForMode (:288-320): mode switch
 *    re-expands the range (WORD → word boundaries) — not yet mirrored here.
 *  - termlib UrlDetection.trimDetectedUrl (:12-35): trailing-punctuation trim
 *    (`,.;:!`) + bracket-pair counting for `)`/`]` — NOT yet mirrored:
 *    `https://x.com/a).` currently selects the trailing `).` (P0 gap, round-217).
 */
object SelectionExpander {

    /**
     * Expand a tap at [col] on [line] to a (startCol, endCol) range,
     * clamped to the line bounds.
     *
     * Rules:
     *  - URL scheme (`scheme://`) expands to the whole URL run.
     *  - Otherwise the maximal non-whitespace run around the tap.
     *  - An enclosing quote/bracket pair is trimmed on the tapped side
     *    so `"hello"` selects `hello` and `(word)` selects `word`.
     */
    fun expandBounds(line: String, col: Int): Pair<Int, Int> {
        if (line.isEmpty()) return 0 to 0
        val c = col.coerceIn(0, line.length - 1)
        // URL: find the start of a `scheme://` token ending at or before c.
        val urlStart = findUrlStart(line, c)
        if (urlStart >= 0) {
            var end = urlStart
            while (end < line.length && !line[end].isWhitespace()) end++
            // Trim trailing prose punctuation from the URL run
            // (termux-app/termlib UrlDetection.trimDetectedUrl): a URL
            // followed by `,`/`.`/`;`/`:`/`!` in prose should not include
            // that punctuation. Balanced-bracket check: `https://x.com/a)`
            // with no matching `(` trims the `)`; `(https://x.com/a)` keeps it.
            var trimmedEnd = end - 1
            while (trimmedEnd > urlStart) {
                val ch = line[trimmedEnd]
                val shouldTrim = ch in TRAILING_URL_PUNCTUATION ||
                    // Include the current char in the bracket count: a `)`
                    // with no matching `(` BEFORE it (including itself) is
                    // prose, not a URL close.
                    (ch == ')' && countOpenLessThanClose(line, trimmedEnd + 1, '(', ')')) ||
                    (ch == ']' && countOpenLessThanClose(line, trimmedEnd + 1, '[', ']'))
                if (!shouldTrim) break
                trimmedEnd--
            }
            return urlStart to trimmedEnd.coerceAtLeast(urlStart)
        }
        return expandWord(line, c)
    }

    /** Prose punctuation that terminates a URL (termlib UrlDetection.kt:9). */
    private val TRAILING_URL_PUNCTUATION = setOf('.', ',', ';', ':', '!')

    /** True when [s] has more `closeChar` than `openChar` in [0, end). */
    private fun countOpenLessThanClose(s: String, end: Int, openChar: Char, closeChar: Char): Boolean {
        var openCount = 0
        var closeCount = 0
        for (i in 0 until end) {
            if (s[i] == openChar) {
                openCount++
            } else if (s[i] == closeChar) {
                closeCount++
            }
        }
        return openCount < closeCount
    }

    private fun findUrlStart(line: String, c: Int): Int {
        // Search backwards for "://" whose scheme starts at a word
        // boundary (start of line or whitespace).
        var i = c
        while (i >= 0) {
            if (i >= 2 && line[i] == '/' && line[i - 1] == '/' && line[i - 2] == ':') {
                // Walk back over the scheme name.
                var schemeStart = i - 3
                while (schemeStart >= 0 && !line[schemeStart].isWhitespace()) schemeStart--
                val start = schemeStart + 1
                // The tap must be at or after the scheme (or at least
                // within the URL run); also require the char before the
                // scheme to be whitespace/start.
                if (c >= start) return start
            }
            if (line[i].isWhitespace()) break
            i--
        }
        return -1
    }

    /** Maximal non-whitespace run around [c], trimmed of enclosing quotes/brackets. */
    private fun expandWord(line: String, c: Int): Pair<Int, Int> {
        var start = c
        var end = c
        while (start > 0 && !line[start - 1].isWhitespace()) start--
        while (end < line.length - 1 && !line[end + 1].isWhitespace()) end++
        // Trim an enclosing quote/bracket pair so `"hello"` selects
        // `hello` (smart-selection behavior, round-214). Each side is
        // trimmed independently: `"hello` (opening quote, tap inside)
        // selects `hello`, `world"` selects `world`.
        if (end > start && line[start] in charArrayOf('"', '\'', '(', '[') && c > start) {
            start++
        }
        if (end > start && line[end] in charArrayOf('"', '\'', ')', ']') && c < end) {
            end--
        }
        return start to end
    }

    /**
     * Strip trailing whitespace AND NUL padding that the renderer uses for
     * empty cells (torvox CellData fast path, round-224).
     */
    private fun String.trimTerminalPadding(): String = trimEnd { it.isWhitespace() || it == '\u0000' }

    /**
     * Whether a character commonly appears inside URLs (Haven's
     * SelectionToolbar.isUrlSafe; termlib marks it internal so Haven
     * duplicates it — we do the same for offline testability).
     */
    private fun Char.isUrlSafe(): Boolean = isLetterOrDigit() || this in "/:@!$&'()*+,;=-._~%?#[]"

    /**
     * Bounds of the full URL a single-row word belongs to, walked outward
     * across wrap-continuation rows. Returns null if the word doesn't extend
     * beyond its row, or if the joined text doesn't look like a URL.
     *
     * Mirrors Haven SelectionToolbar.expandAcrossUrlWrap (:120-214):
     *  - backward: while the tracked word is the first non-whitespace run on
     *    its row and the previous row's last non-padding char is URL-safe.
     *  - forward: while at the row edge and the next row starts with a
     *    URL-safe run; an indented run is a wrap tail only when it's a single
     *    short run (hanging-indent shape), not indented prose.
     *  - joined text must look like a full URL (`://` or `www.` prefix with a
     *    dotted host) — the final gate against sprawling into prose.
     */
    internal data class UrlSpan(
        val startRow: Int,
        val startCol: Int,
        val endRow: Int,
        val endCol: Int,
    )

    @JvmStatic
    internal fun looksLikeFullUrl(s: String): Boolean = URL_TOKEN_RE.matches(s)

    @JvmStatic
    internal fun expandAcrossUrlWrap(
        lines: List<String>,
        row: Int,
        wordStartCol: Int,
        wordEndCol: Int,
        columns: Int = 0,
    ): UrlSpan? {
        val currentText = lines.getOrNull(row) ?: return null
        val trimmedLen = currentText.trimTerminalPadding().length

        var startRow = row
        var startCol = wordStartCol
        var endRow = row
        var endCol = wordEndCol

        val backward = walkBackwardAcrossWraps(lines, row, startRow, startCol)
        startRow = backward.first
        startCol = backward.second

        val forward = walkForwardAcrossWraps(lines, row, endRow, endCol, trimmedLen, columns)
        endRow = forward.first
        endCol = forward.second

        // No continuation found → keep single-row word.
        if (startRow == row && endRow == row) return null

        // Verify the joined text actually looks like a URL before overriding
        // the walk. Continuation rows contribute from their first
        // non-whitespace column so a hanging indent never leaks in.
        val joined = buildString {
            for (r in startRow..endRow) {
                val line = lines[r]
                val from =
                    if (r == startRow) {
                        startCol
                    } else {
                        line.indexOfFirst { !it.isWhitespace() }.coerceAtLeast(0)
                    }
                val toExclusive =
                    if (r == endRow) {
                        (endCol + 1).coerceAtMost(line.length)
                    } else {
                        line.trimTerminalPadding().length
                    }
                if (from < toExclusive) append(line.substring(from, toExclusive))
            }
        }
        if (!looksLikeFullUrl(joined)) return null

        return UrlSpan(startRow, startCol, endRow, endCol)
    }

    /**
     * Walk backward into previous rows while the tracked word is the first
     * non-whitespace run on its row and the previous row's last non-padding
     * character is URL-safe. Returns the walked (row, col).
     */
    private fun walkBackwardAcrossWraps(
        lines: List<String>,
        row: Int,
        startRow: Int,
        startCol: Int,
    ): Pair<Int, Int> {
        var currentRow = startRow
        var currentCol = startCol
        while (currentRow > 0) {
            if (lines[currentRow].indexOfFirst { !it.isWhitespace() } != currentCol) break
            val prev = lines[currentRow - 1].trimTerminalPadding()
            if (prev.isEmpty() || !prev.last().isUrlSafe()) break
            var s = prev.length
            while (s > 0 && !prev[s - 1].isWhitespace() && prev[s - 1].isUrlSafe()) s--
            currentRow -= 1
            currentCol = s
        }
        return currentRow to currentCol
    }

    /**
     * Walk forward into next rows while we're at this row's trimmed edge and
     * the next row contributes a URL-safe run. A column-0 run is a classic
     * soft-wrap; an indented run is a wrap tail only when it's a single short
     * run on an otherwise-blank line (hanging-indent shape), not indented
     * prose or a table cell. Returns the walked (row, col).
     */
    private fun walkForwardAcrossWraps(
        lines: List<String>,
        row: Int,
        endRow: Int,
        endCol: Int,
        trimmedLen: Int,
        columns: Int,
    ): Pair<Int, Int> {
        var currentRow = endRow
        var currentCol = endCol
        while (currentCol >= trimmedLen - 1 && currentRow < lines.size - 1) {
            val next = lines[currentRow + 1]
            val t0 = next.indexOfFirst { !it.isWhitespace() }
            if (t0 < 0 || !next[t0].isUrlSafe()) break
            var e = t0
            while (e < next.length && !next[e].isWhitespace() && next[e].isUrlSafe()) e++
            if (t0 > 0) {
                val afterRun = next.substring(e)
                val runLen = e - t0
                val width = if (columns > 0) columns else next.length
                if (afterRun.isNotBlank() || runLen > (width / 2).coerceAtLeast(1)) break
            }
            currentRow += 1
            currentCol = (e - 1).coerceAtLeast(t0)
        }
        return currentRow to currentCol
    }

    /**
     * Whether a whitespace-free string looks like a *complete* URL token
     * (scheme- or `www.`-prefixed, a dotted host, optional path/query). Pure
     * Kotlin on purpose — `android.util.Patterns.WEB_URL`'s class initializer
     * isn't available in plain JVM unit tests.
     */
    private val URL_TOKEN_RE =
        Regex(
            "^(?:https?://|www\\.)[\\w-]+(?:\\.[\\w-]+)+(?:[/:?#@!\$&'()*+,;=._~%\\[\\]-]\\S*)?$",
            RegexOption.IGNORE_CASE,
        )
}
