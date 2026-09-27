package terminal.emulator.bridge

/**
 * 终端内容的 Kotlin 侧查询端口。实际实现是 [NativeQueryPort]（JNI）。
 *
 * 对调用方的约定：
 * - `scrollbackLine`/`scrollbackLength`/`searchAllInScrollback` 返回 null/0/空列表即「无数据」，
 *   应视为不可用而非「内容为空」——伪造数据会损坏选区与搜索结果。
 * - `isCellEmpty` 返回 true 时长按弹出粘贴菜单（无原生数据时唯一可用的长按动作）。
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
    fun isCellEmpty(row: Int, col: Int): Boolean
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
    fun getDefaultFontName(): String
    fun getFontInfo(): String?
}
