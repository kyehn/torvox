package terminal.emulator.ui

import androidx.compose.ui.graphics.Color
import java.io.ByteArrayOutputStream

/**
 * 把搜索命中打包为原生高亮载荷。
 *
 * 线格式（Rust 侧 `parse_search_highlights`，二者必须逐字一致）：
 * `[计数(i32 LE)][记录…]`，每条记录 16 字节 —— `row`/`start_col`/`end_col` 各一个
 * i32 LE，末尾 4 字节 RGBA。
 *
 * 计数前缀是**本批实际写出的记录数**，不是全部命中数：只有落在视口内的命中才被写成
 * 记录，故前缀必须与记录数相等。声明全量命中数会让原生按「声明条数多于完整记录数」
 * 判为畸形载荷并整批丢弃，高亮随之全不生效（见 Rust 侧 `CountExceedsRecords`）。
 */
object SearchHighlightPacker {
    /** 记录内整数字段个数：`row`、`start_col`、`end_col`。 */
    private const val FIELD_COUNT = 3

    /** 单个整数字段的字节数。 */
    private const val FIELD_BYTES = 4

    /** 记录尾部 RGBA 的字节数。 */
    private const val COLOR_BYTES = 4

    /** 单条记录的字节数。 */
    private const val RECORD_BYTES = FIELD_COUNT * FIELD_BYTES + COLOR_BYTES

    /** 计数前缀的字节数。 */
    private const val COUNT_BYTES = FIELD_BYTES

    /**
     * 只保留落在视口内的命中，并把绝对回滚行折算为网格行。
     *
     * 折算口径与渲染侧一致：`gridRow = 回滚行号 - 回滚长度 + 视口滚动偏移`，
     * 故行号随滚动即时变化（见调用处的滚动偏移镜像）。
     */
    private fun visibleMatches(
        results: List<SearchResult>,
        currentIndex: Int,
        rows: Int,
        scrollbackCount: Int,
        scrollOffset: Int,
    ): List<Triple<Int, SearchResult, Boolean>> = results.withIndex().mapNotNull { (index, match) ->
        val gridRow = match.lineIndex - scrollbackCount + scrollOffset
        if (gridRow < 0 || gridRow >= rows) {
            null
        } else {
            Triple(gridRow, match, index == currentIndex)
        }
    }

    /**
     * 打包高亮载荷。视口外命中不写记录，故返回载荷可能比全部命中数短。
     *
     * @param foreground 当前命中项的高亮色（完全不透明的主题前景色）。
     * @param selectionBackground 其余命中项的高亮色（降低不透明度的主题选区色）。
     */
    fun pack(
        results: List<SearchResult>,
        currentIndex: Int,
        rows: Int,
        scrollbackCount: Int,
        scrollOffset: Int,
        foreground: Color,
        selectionBackground: Color,
    ): ByteArray {
        val buffer = ByteArrayOutputStream()

        fun writeI32(value: Int) {
            for (byteIndex in 0 until FIELD_BYTES) {
                buffer.write(value ushr (byteIndex * 8) and 0xFF)
            }
        }

        fun writeByte(value: Int) {
            buffer.write(value)
        }

        val visible = visibleMatches(results, currentIndex, rows, scrollbackCount, scrollOffset)
        writeI32(visible.size)
        for ((gridRow, match, isCurrent) in visible) {
            writeI32(gridRow)
            writeI32(match.startIndex)
            // 高亮至少覆盖一个单元格：零宽命中（start == end）在渲染侧无从命中任何格。
            writeI32(match.endIndex.coerceAtLeast(match.startIndex + 1))
            val color = if (isCurrent) foreground else selectionBackground
            writeByte((color.red * 255).toInt())
            writeByte((color.green * 255).toInt())
            writeByte((color.blue * 255).toInt())
            writeByte(
                if (isCurrent) SearchHighlightColors.CURRENT_MATCH_ALPHA else SearchHighlightColors.OTHER_MATCH_ALPHA,
            )
        }
        return buffer.toByteArray()
    }
}
