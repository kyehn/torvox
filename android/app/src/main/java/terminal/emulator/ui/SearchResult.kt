package terminal.emulator.ui

/** 终端回滚缓冲中的一次搜索命中。 */
data class SearchResult(val lineIndex: Int, val startIndex: Int, val endIndex: Int) {
    companion object {
        /**
         * 新查询是否为旧查询的收窄（变短且是旧串的子串）。
         * 按 GNOME Console（kgx）g_strrstr 的子串包含语义判定，而非仅前缀匹配。
         */
        fun isNarrowingDown(query: String, previousQuery: String): Boolean = query.isNotEmpty() &&
            previousQuery.isNotEmpty() &&
            query.length < previousQuery.length &&
            previousQuery.contains(query)

        /**
         * 上一个/下一个导航索引（含回绕，对标参考搜索步进语义）。
         * 空结果返回负一，调用方不得索引。
         */
        fun nextIndex(currentIndex: Int, resultCount: Int): Int {
            if (resultCount <= 0) return -1
            return if (currentIndex < resultCount - 1) currentIndex + 1 else 0
        }

        fun previousIndex(currentIndex: Int, resultCount: Int): Int {
            if (resultCount <= 0) return -1
            return if (currentIndex > 0) currentIndex - 1 else resultCount - 1
        }
    }
}
