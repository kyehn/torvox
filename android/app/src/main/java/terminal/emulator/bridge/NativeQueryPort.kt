package terminal.emulator.bridge

/** 原生搜索结果扁平数组中每项的 `Int` 个数：row、start_col、end_col。 */
private const val MATCH_STRIDE = 3

/**
 * 原生查询路径：每个方法与 JNI 导出 1:1 对应，供 [Bridge] 查询活动会话。
 *
 * 对调用方的约定：null/0/空表示引擎「无数据」，绝不可伪造。
 * 单行/字体查询开销小，批量查询（[getTerminalText]、[searchAllInScrollback]）由 UI 负责防抖。
 */
class NativeQueryPort(private val sessionIdProvider: () -> Long) {
    fun getTitle(): String? = NativeBridge.getTitle(sessionIdProvider())

    fun getActiveSessionTitle(): String = getTitle() ?: ""

    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?) {
        // 选区由终端侧持有（引用跟踪，经 NativeBridge.setSelection 安装）：
        // VT 线程把反色烘焙进 CellData，视图侧无需维护单元格簿记。
        val active = hasSelection ?: true
        NativeBridge.setSelection(
            sessionIdProvider(),
            startRow,
            startCol,
            endRow,
            endCol,
            active,
        )
    }

    fun clearSearchHighlights() {
        NativeBridge.clearSearchHighlights(sessionIdProvider())
    }

    fun setSearchHighlights(data: ByteArray) {
        NativeBridge.setSearchHighlights(sessionIdProvider(), data)
    }

    fun scrollbackLine(row: Int): String? = NativeBridge.scrollbackLine(sessionIdProvider(), row)

    fun scrollbackLength(): Int = NativeBridge.scrollbackLength(sessionIdProvider())

    fun cursorViewportPacked(): Long = NativeBridge.getCursorViewportPacked(sessionIdProvider())

    fun searchAllInScrollback(query: String, caseSensitive: Boolean): List<Triple<Int, Int, Int>>? =
        NativeBridge.searchAllInScrollback(
            sessionIdProvider(),
            query,
            caseSensitive,
        )?.let(::searchMatches)

    fun setScrollOffset(offset: Int) {
        // 原生侧在 VT 线程经 scroll_viewport 应用增量，下一次 CellData 推送即带上滚动后的视图。
        NativeBridge.setScrollOffset(sessionIdProvider(), offset)
    }

    fun setScrollYPx(offsetPx: Float) {
        NativeBridge.setScrollYPx(sessionIdProvider(), offsetPx)
    }

    fun getTerminalText(): String? = NativeBridge.getTerminalText(sessionIdProvider())

    fun listFontFamilies(): List<String>? = NativeBridge.listFontFamilies()?.toList()

    fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String? = NativeBridge.selectionText(
        sessionIdProvider(),
        startRow,
        startCol,
        endRow,
        endCol,
    )

    fun hyperlinkAt(row: Int, col: Int): String? = NativeBridge.hyperlinkAt(sessionIdProvider(), row, col)

    fun wideCharTailCols(row: Int): IntArray = NativeBridge.wideCharTailCols(sessionIdProvider(), row)

    fun selectWordAt(row: Int, col: Int): IntArray? = NativeBridge.selectWordAt(sessionIdProvider(), row, col)

    fun selectAll(): IntArray? = NativeBridge.selectAll(sessionIdProvider())

    fun getDefaultFontName(): String = NativeBridge.getDefaultFontName() ?: ""

    fun getFontInfo(): String? = NativeBridge.getFontInfo()
}

/**
 * 原生扁平搜索结果 → `(row, start_col, end_col)` 三元组列表。
 *
 * 不做范围校验：原生在构造命中时即以 `start < end` 把关（`end_col` 必然大于
 * `start_col`），三者是 `u32` 故不可能为负。此处再过滤只会让无效区间被静默吞成
 * 「无结果」，把生产端的问题藏起来。
 */
internal fun searchMatches(flat: IntArray): List<Triple<Int, Int, Int>> = List(flat.size / MATCH_STRIDE) { index ->
    val base = index * MATCH_STRIDE
    Triple(flat[base], flat[base + 1], flat[base + 2])
}
