package terminal.emulator.util

// wcwidth-style wide-character classification (CJK, emoji, Hangul).
// Cell width rules: narrow (width 1) vs wide (width 2) code points, from
// Markus Kuhn's wcwidth() tables. Single source of truth for terminal cell
// width — previously duplicated in TerminalSurface, TextSearchBar and
// TerminalViewModel (deduplicated; see docs/rejected-technologies.md §9).

/** BMP wide ranges from Markus Kuhn's wcwidth() tables. */
fun isWideBmp(cp: Int): Boolean = cp in 0x1100..0x115F || // Hangul Jamo
    cp in 0x2329..0x232A || // angle brackets
    cp in 0x2E80..0x303E || // CJK Radicals Supplement .. CJK Symbols and Punctuation
    cp in 0x3041..0x33FF || // Hiragana .. CJK Compatibility
    cp in 0x3400..0x4DBF || // CJK Unified Ideographs Extension A
    cp in 0x4E00..0x9FFF || // CJK Unified Ideographs
    cp in 0xA000..0xA4CF || // Yi Syllables
    cp in 0xAC00..0xD7A3 || // Hangul Syllables
    cp in 0xF900..0xFAFF || // CJK Compatibility Ideographs
    cp in 0xFE30..0xFE4F || // CJK Compatibility Forms
    cp in 0xFF00..0xFF60 || // Fullwidth Forms
    cp in 0xFFE0..0xFFE6 // Fullwidth Signs

/** Astral-plane wide ranges (emoji and CJK extensions B-G). */
fun isWideAstral(cp: Int): Boolean = cp in 0x1F1E6..0x1F1FF || // Regional Indicator (flag)
    cp in 0x1F300..0x1F64F || // Emoticons
    cp in 0x1F680..0x1F6FF || // Transport and Map Symbols
    cp in 0x1F700..0x1F8FF || // Alchemical .. Geometric Extended
    cp in 0x1F900..0x1F9FF || // Supplemental Symbols
    cp in 0x1FA00..0x1FAFF || // Chess .. Symbols Extended-A
    cp in 0x20000..0x2FFFD || // CJK Extensions B-F
    cp in 0x30000..0x3FFFD // CJK Extension G

/** True when the code point occupies two terminal cells. */
fun isWideCodePoint(cp: Int): Boolean = isWideBmp(cp) || isWideAstral(cp)

/** True when the character occupies two terminal cells. */
fun isWideChar(ch: Char): Boolean = isWideCodePoint(ch.code)

/** Terminal cell width of a character: 1 or 2. */
fun charCellWidth(ch: Char): Int = if (isWideChar(ch)) 2 else 1

/** Cell column of the character at [charIndex] on [line], summing cell widths. */
fun charIndexToCellColumn(
    line: String,
    charIndex: Int,
): Int {
    var col = 0
    for (i in 0 until charIndex.coerceAtMost(line.length)) {
        col += charCellWidth(line[i])
    }
    return col
}
