package terminal.emulator.bridge

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import terminal.emulator.runtime.LogUtil

private const val TAG = "NativeQueryPort"

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
        )
            ?.let { parseSearchMatches(it) }

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

@Serializable
internal data class SearchMatchDto(val row: Int = 0, val start_col: Int = 0, val end_col: Int = 0)

/**
 * 解析原生 `searchAllInScrollback` 导出的 `{"row":int,"start_col":int,"end_col":int}` JSON 数组。
 * 输入非法时返回空列表而不抛异常，使搜索降级为「无结果」而非崩溃 UI——但必须记日志：
 * 否则解码失败与「真的没有匹配」在 UI 上完全同形，日志里也无迹可寻。
 * 丢弃不可能的范围（负值、end <= start），否则缺失字段会静默产生 row=0 的伪命中而高亮错行。
 */
private val searchJson = Json { ignoreUnknownKeys = true }

internal fun parseSearchMatches(json: String): List<Triple<Int, Int, Int>> = try {
    searchJson
        .decodeFromString<List<SearchMatchDto>>(json)
        .filter { it.row >= 0 && it.start_col >= 0 && it.end_col > it.start_col }
        .map { Triple(it.row, it.start_col, it.end_col) }
} catch (error: Exception) {
    // 不吞：解码失败必须与「无匹配」可区分，否则搜索坏掉时用户与维护者都无从判断。
    LogUtil.w(TAG, "search result decode failed", error)
    emptyList()
}
