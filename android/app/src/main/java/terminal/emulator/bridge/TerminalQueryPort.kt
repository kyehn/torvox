package terminal.emulator.bridge

/**
 * 终端内容的 Kotlin 侧查询端口。实际实现是 [NativeQueryPort]（JNI）。
 *
 * 对调用方的约定：
 * - `scrollbackLine`/`scrollbackLength`/`searchAllInScrollback` 返回 null/0/空列表即「无数据」，
 *   应视为不可用而非「内容为空」——伪造数据会损坏选区与搜索结果。
 */
// 查询面刻意保持宽接口：与原生导出一一对应，使接缝可替换而不牵动 UI。
interface TerminalQueryPort {
    fun getTitle(): String?
    fun getActiveSessionTitle(): String = getTitle() ?: ""

    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean? = null)

    fun clearSearchHighlights()
    fun setSearchHighlights(data: ByteArray)
    fun scrollbackLine(row: Int): String?
    fun scrollbackLength(): Int

    /** 光标视口位置，打包为 `(y << 32) | x`，隐藏时为 -1。 */
    fun cursorViewportPacked(): Long
    fun searchAllInScrollback(query: String, caseSensitive: Boolean): List<Triple<Int, Int, Int>>?
    fun setScrollOffset(offset: Int)

    /** 视口 Y 像素余量，用于逐像素平滑滚动（正值 = 内容下移）。 */
    fun setScrollYPx(offsetPx: Float)

    fun getTerminalText(): String?
    fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String?
    fun listFontFamilies(): List<String>?

    // 上游选择派生（native select_word/select_line/select_all）：native 侧
    // 已把选区安装为终端状态，回传有序界限 [startRow, startCol, endRow,
    // endCol]（绝对网格坐标）。null = 无可选内容，按“无数据”处理，勿伪造。
    fun selectWordAt(row: Int, col: Int): IntArray?
    fun selectLineAt(row: Int, col: Int): IntArray?
    fun selectAll(): IntArray?

    fun hyperlinkAt(row: Int, col: Int): String?

    /**
     * 网格列 → 该列所属字符的起始列：宽字符尾格左移一格，其余原样返回。
     *
     * [scrollbackLine] 每列恰好一个字符（宽字符尾格为空格占位），行文本因此**无法**
     * 区分尾格与真空白，吸附判定只能取自网格单元宽度这一事实。会话不存在或查询
     * 失败时原生返回传入的列，不猜。
     */
    fun cellCharStartCol(row: Int, col: Int): Int
    fun getDefaultFontName(): String
    fun getFontInfo(): String?
}
