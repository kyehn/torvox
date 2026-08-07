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
}
