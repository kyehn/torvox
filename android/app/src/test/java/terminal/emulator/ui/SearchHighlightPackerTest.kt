package terminal.emulator.ui

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 高亮载荷线格式的回归测试。Rust 侧 `parse_search_highlights` 在「声明条数多于完整
 * 记录数」时整批拒收，故 Kotlin 侧的计数前缀与记录数必须恒等。
 *
 * 坐标口径：`gridRow = 回滚行号 - (回滚长度 - 滚动偏移)`，即相对视口顶的行号；
 * `scrollOffset` 为 0 表示贴底（视口顶行 = 回滚长度），随上滑递增。
 */
class SearchHighlightPackerTest {
    private val currentColor = Color(0xFFEEEEEE)
    private val otherColor = Color(0xFF445566)

    private fun declaredCount(payload: ByteArray): Int = intField(payload, 0, -1)

    private fun recordCount(payload: ByteArray): Int = (payload.size - 4) / 16

    /** 读载荷内第 [recordIndex] 条记录的第 [fieldIndex] 个整数字段（`-1` 为计数前缀）。 */
    private fun intField(payload: ByteArray, recordIndex: Int, fieldIndex: Int): Int {
        val start = 4 + recordIndex * 16 + fieldIndex * 4
        return (payload[start].toInt() and 0xFF) or
            ((payload[start + 1].toInt() and 0xFF) shl 8) or
            ((payload[start + 2].toInt() and 0xFF) shl 16) or
            ((payload[start + 3].toInt() and 0xFF) shl 24)
    }

    private fun alpha(payload: ByteArray, recordIndex: Int): Int = payload[4 + recordIndex * 16 + 15].toInt() and 0xFF

    private fun red(payload: ByteArray, recordIndex: Int): Int = payload[4 + recordIndex * 16 + 12].toInt() and 0xFF

    @Test
    fun `declared count equals written records when matches fall outside the viewport`() {
        // 回滚 100 行、视口 20 行、贴底：可见回滚行 100..119。
        // 三条命中中两条在视口外 —— 声明全量 3 条会让原生整批拒收。
        val results = listOf(
            SearchResult(lineIndex = 5, startIndex = 0, endIndex = 3),
            SearchResult(lineIndex = 110, startIndex = 2, endIndex = 5),
            SearchResult(lineIndex = 150, startIndex = 1, endIndex = 4),
        )
        val payload =
            SearchHighlightPacker.pack(
                results = results,
                currentIndex = 1,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 0,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(1, recordCount(payload))
        assertEquals(recordCount(payload), declaredCount(payload))
    }

    @Test
    fun `every match inside the viewport keeps its record`() {
        val results = listOf(
            SearchResult(lineIndex = 105, startIndex = 0, endIndex = 3),
            SearchResult(lineIndex = 110, startIndex = 2, endIndex = 5),
            SearchResult(lineIndex = 115, startIndex = 1, endIndex = 4),
        )
        val payload =
            SearchHighlightPacker.pack(
                results = results,
                currentIndex = 0,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 0,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(3, recordCount(payload))
        assertEquals(3, declaredCount(payload))
    }

    @Test
    fun `no match inside the viewport yields a legal empty payload`() {
        val results = listOf(SearchResult(lineIndex = 0, startIndex = 0, endIndex = 3))
        val payload =
            SearchHighlightPacker.pack(
                results = results,
                currentIndex = 0,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 0,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(4, payload.size)
        assertEquals(0, declaredCount(payload))
    }

    @Test
    fun `scrollback row is converted to grid row and scrolls out of view`() {
        // 回滚行 110、回滚长度 100：贴底时网格行 10（上滑 10 行后仍在视口内）。
        val results = listOf(SearchResult(lineIndex = 110, startIndex = 2, endIndex = 5))

        val atBottom =
            SearchHighlightPacker.pack(
                results = results,
                currentIndex = 0,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 0,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(1, recordCount(atBottom))
        assertEquals(10, intField(atBottom, 0, 0))
        assertEquals(2, intField(atBottom, 0, 1))
        assertEquals(5, intField(atBottom, 0, 2))

        // 上滑 20 行：视口顶推到回滚行 80，原行 110 落到网格行 30，移出 20 行视口。
        val scrolledAway =
            SearchHighlightPacker.pack(
                results = results,
                currentIndex = 0,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 20,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(0, recordCount(scrolledAway))
        assertEquals(0, declaredCount(scrolledAway))
    }

    @Test
    fun `current match carries the opaque foreground and others the dimmed selection color`() {
        val results = listOf(
            SearchResult(lineIndex = 110, startIndex = 0, endIndex = 3),
            SearchResult(lineIndex = 111, startIndex = 0, endIndex = 3),
        )
        val payload =
            SearchHighlightPacker.pack(
                results = results,
                currentIndex = 1,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 0,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(SearchHighlightColors.OTHER_MATCH_ALPHA, alpha(payload, 0))
        assertEquals(SearchHighlightColors.CURRENT_MATCH_ALPHA, alpha(payload, 1))
        assertEquals(0x44, red(payload, 0))
        assertEquals(0xEE, red(payload, 1))
    }

    @Test
    fun `zero width match still covers one cell`() {
        // 零宽命中在渲染侧无从命中任何格，故端点被抬到 start+1。
        val payload =
            SearchHighlightPacker.pack(
                results = listOf(SearchResult(lineIndex = 110, startIndex = 4, endIndex = 4)),
                currentIndex = 0,
                rows = 20,
                scrollbackCount = 100,
                scrollOffset = 0,
                foreground = currentColor,
                selectionBackground = otherColor,
            )
        assertEquals(4, intField(payload, 0, 1))
        assertEquals(5, intField(payload, 0, 2))
        assertTrue(intField(payload, 0, 2) > intField(payload, 0, 1))
    }
}
