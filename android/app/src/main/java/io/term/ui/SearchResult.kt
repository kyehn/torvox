package io.term.ui

/**
 * Represents a single match found during text search within the terminal scrollback.
 *
 * @property lineIndex Row index in the terminal grid where the match occurs.
 * @property startIndex Column index of the first matching character.
 * @property endIndex Column index after the last matching character.
 */
data class SearchResult(
    val lineIndex: Int,
    val startIndex: Int,
    val endIndex: Int,
)
