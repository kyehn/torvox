package terminal.emulator.bridge

/** 一次搜索命中的网格区间：`row` 是绝对网格行（0 = 回滚顶部），`endCol` 为开区间上界。 */
data class SearchMatchRange(val row: Int, val startCol: Int, val endCol: Int)

/** 搜索选中态：命中序号（0 = 最新）、命中总数与选中命中的区间。 */
data class SearchSelection(val index: Int, val total: Int, val matched: SearchMatchRange)

/**
 * 原生查询路径：每个方法与 JNI 导出 1:1 对应，供 [Bridge] 查询活动会话。
 *
 * 对调用方的约定：null/0/空表示引擎「无数据」，绝不可伪造。
 * 单行/字体查询开销小，批量查询（[getTerminalText]、[searchSetNeedle]）由 UI 负责防抖。
 */
class NativeQueryPort(private val sessionIdProvider: () -> Long) {
    fun getTitle(): String? = NativeBridge.getTitle(sessionIdProvider())

    /**
     * 会话的工作目录（OSC 7）；shell 尚未上报时为空串。
     *
     * 空串不在这里被替换成任何「默认目录」：调用方据「空」判定「还不知道」并保留
     * 上一次已知值，替换成猜测值会让标题栏在每次切会话时闪回 home。
     */
    fun getPwd(): String = NativeBridge.getPwd(sessionIdProvider()).orEmpty()

    fun getActiveSessionTitle(): String = getTitle() ?: ""

    /**
     * 安装选区；返回 false 表示本次写入被推迟（会话锁被 PTY 写入持有），
     * 调用方保留旧快照、下一帧重试。
     */
    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?): Boolean {
        // 选区由终端侧持有（引用跟踪，经 NativeBridge.setSelection 安装）：
        // VT 线程把反色烘焙进 CellData，视图侧无需维护单元格簿记。
        val active = hasSelection ?: true
        return NativeBridge.setSelection(
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

    fun applySearchHighlights(scrollOffset: Int) {
        NativeBridge.applySearchHighlights(sessionIdProvider(), scrollOffset)
    }

    fun scrollbackLine(row: Int): String? = NativeBridge.scrollbackLine(sessionIdProvider(), row)

    fun scrollbackLength(): Int = NativeBridge.scrollbackLength(sessionIdProvider())

    fun renderedScrollbackLength(): Int = NativeBridge.getRenderedScrollbackLength(sessionIdProvider())

    fun cursorViewportPacked(): Long = NativeBridge.getCursorViewportPacked(sessionIdProvider())

    fun renderedCursorPacked(): Long = NativeBridge.getRenderedCursorPacked(sessionIdProvider())

    fun visibleText(): String = NativeBridge.getVisibleText(sessionIdProvider()).orEmpty()

    fun searchSetNeedle(needle: String): Int = NativeBridge.searchSetNeedle(sessionIdProvider(), needle)

    fun searchSelect(forward: Boolean): SearchSelection? =
        NativeBridge.searchSelect(sessionIdProvider(), forward)?.let { flat ->
            SearchSelection(
                index = flat[0],
                total = flat[1],
                matched = SearchMatchRange(flat[2], flat[3], flat[4]),
            )
        }

    /**
     * 原生侧在 VT 线程经 scroll_viewport 应用增量，下一次 CellData 推送即带上滚动后的视图。
     * 返回 false 表示写入被推迟（会话锁被 PTY 写入持有）：调用方保留旧值、下一帧重试。
     */
    fun setScrollOffset(offset: Int): Boolean = NativeBridge.setScrollOffset(sessionIdProvider(), offset)

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

    fun getFontInfo(): ByteArray? = NativeBridge.getFontInfo()
}
