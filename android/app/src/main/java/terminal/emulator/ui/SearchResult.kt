package terminal.emulator.ui

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
) {
    companion object {
        /**
         * Determines whether the current search query is a "narrowing" of the previous query.
         *
         * Narrowing means the user has shortened the search string (e.g., by deleting characters)
         * and the new shorter query is a substring of the previous query. When narrowing, the
         * current match index should be preserved (clamped to valid range) rather than resetting
         * to 0, so the user stays on or near their previous position.
         *
         * This mirrors GNOME Console (kgx) behavior where `g_strrstr(last_search, search)`
         * checks for substring containment — not just prefix matching.
         *
         * @param query The current (shorter) search query.
         * @param previousQuery The previous (longer) search query.
         * @return true if the current query narrows the previous query.
         */
        fun isNarrowingDown(
            query: String,
            previousQuery: String,
        ): Boolean = query.isNotEmpty() &&
            previousQuery.isNotEmpty() &&
            query.length < previousQuery.length &&
            previousQuery.contains(query)
    }
}
