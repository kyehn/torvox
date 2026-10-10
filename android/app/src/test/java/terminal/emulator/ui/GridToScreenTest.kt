package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure grid→pixel coordinate conversion extracted from TerminalSurface.
 * Covers the combinations that regressed historically: scrolling,
 * search-jump and font-size changes all feed the same formula, plus the
 * corner cases (zero scroll / max scroll / negative-offset clamping).
 */
class GridToScreenTest {

    private val epsilon = 1e-4f

    private fun assertPoint(actual: Pair<Float, Float>, expectedX: Float, expectedY: Float) {
        assertEquals(expectedX, actual.first, epsilon)
        assertEquals(expectedY, actual.second, epsilon)
    }

    // ── zero scroll ───────────────────────────────────────────────────────

    @Test
    fun `no scroll maps grid origin to viewport origin`() {
        assertPoint(
            gridToScreen(row = 0, col = 0, viewportTopGrid = 0, cellWidth = 10f, cellHeight = 20f),
            0f,
            0f,
        )
    }

    @Test
    fun `no scroll maps every grid row linearly downward`() {
        // scrollOffset = 0 ⇒ viewportTopGrid = 0: absolute rows are viewport rows.
        for (row in listOf(0, 1, 11, 23)) {
            val p =
                gridToScreen(
                    row = row,
                    col = 0,
                    viewportTopGrid = 0,
                    cellWidth = 9f,
                    cellHeight = 18f,
                )
            assertEquals(row * 18f, p.second, epsilon)
        }
    }

    @Test
    fun `columns map left to right by cell width`() {
        val cw = 12f
        assertEquals(0f, gridToScreen(0, 0, 0, cw, 20f).first, epsilon)
        assertEquals(cw, gridToScreen(0, 1, 0, cw, 20f).first, epsilon)
        // Last column of an 80-col grid.
        assertEquals(79 * cw, gridToScreen(0, 79, 0, cw, 20f).first, epsilon)
    }

    // ── scrolling ─────────────────────────────────────────────────────────

    @Test
    fun `scrolling shifts rows up by exactly viewportTopGrid cells`() {
        val ch = 22f
        // Mid-scroll: 500 lines of history above a 24-row viewport.
        val viewportTopGrid = 500
        assertPoint(
            gridToScreen(row = 512, col = 3, viewportTopGrid = viewportTopGrid, cellWidth = 10f, cellHeight = ch),
            30f,
            12 * ch,
        )
    }

    @Test
    fun `max scroll puts first visible grid row at viewport top`() {
        // scrollbackLength=1000, 24 visible rows, scrolled fully back to row 24.
        val scrollbackLength = 1000
        val visibleRows = 24
        val viewportTopGrid = scrollbackLength - scrollOffsetForRow(visibleRows, scrollbackLength)
        assertPoint(gridToScreen(viewportTopGrid, 0, viewportTopGrid, 10f, 20f), 0f, 0f)
        // The oldest line sits one full viewport above the top row.
        assertPoint(
            gridToScreen(row = 0, col = 7, viewportTopGrid = viewportTopGrid, cellWidth = 10f, cellHeight = 20f),
            70f,
            -visibleRows * 20f,
        )
    }

    @Test
    fun `bottom visible row ends at one viewport height`() {
        val ch = 25f
        val viewportTopGrid = 40
        val lastVisibleRow = viewportTopGrid + 23
        // Row bottom edge (what drag handles anchor to): pass row + 1.
        val (_, bottomY) =
            gridToScreen(
                row = lastVisibleRow + 1,
                col = 0,
                viewportTopGrid = viewportTopGrid,
                cellWidth = 10f,
                cellHeight = ch,
            )
        assertEquals(24 * ch, bottomY, epsilon)
    }

    // ── search jump ───────────────────────────────────────────────────────

    @Test
    fun `search jump lands the hit row at the viewport top`() {
        // 外部真相是 TESTING.md 的「上一个/下一个把命中行滚到对应位置」：跳转后
        // 命中行的顶边必须落在视口顶（y = 0）。偏移走生产用的 scrollOffsetForRow，
        // 不再在测试里复刻一遍公式——复刻时该断言恒成立，删掉 scrollToRow 也判不了红。
        val scrollbackLength = 5000
        val hitRow = 4321
        val viewportTopGrid = scrollbackLength - scrollOffsetForRow(hitRow, scrollbackLength)
        assertPoint(
            gridToScreen(row = hitRow, col = 17, viewportTopGrid = viewportTopGrid, cellWidth = 9f, cellHeight = 19f),
            17 * 9f,
            0f,
        )
    }

    @Test
    fun `search jump near scrollback start keeps rows on screen`() {
        // Hit near the top of the history while scrolled far down: the jump
        // moves viewportTopGrid from 4000 to 5; rows between stay positive.
        val beforeJump =
            gridToScreen(row = 8, col = 2, viewportTopGrid = 4000, cellWidth = 10f, cellHeight = 20f)
        assertPoint(beforeJump, 20f, -3992 * 20f)
        val afterJump =
            gridToScreen(row = 8, col = 2, viewportTopGrid = 5, cellWidth = 10f, cellHeight = 20f)
        assertPoint(afterJump, 20f, 3 * 20f)
    }

    // ── font-size changes ─────────────────────────────────────────────────

    @Test
    fun `font size change rescales both axes linearly`() {
        // 外部真相：缩放只改两轴的像素比例，不改网格点之间的关系。
        // 断言具体像素值（换算结果），而不是把生产表达式原样写一遍——后者对任何
        // 保持输出的结构改动都照样通过，等于没测。网格点换算本身已由本文件前面的
        // 非平凡视口偏移用例钉住。
        val (row, col, viewportTopGrid) = Triple(130, 41, 118)
        // 小 → 大字号：同一个网格点，(x, y) 必须按同一倍率放大。
        val (small, large) = (10f to 20f) to (24f to 48f)
        val pSmall = gridToScreen(row, col, viewportTopGrid, small.first, small.second)
        val pLarge = gridToScreen(row, col, viewportTopGrid, large.first, large.second)
        assertEquals(410f, pSmall.first, epsilon)
        assertEquals(240f, pSmall.second, epsilon)
        assertEquals(984f, pLarge.first, epsilon)
        assertEquals(576f, pLarge.second, epsilon)
        // 两轴比例相同：字号变化不引入非等比缩放（压扁/拉伸的直接来源）。
        assertEquals(pLarge.first / pSmall.first, pLarge.second / pSmall.second, 0.001f)
    }

    @Test
    fun `an offset past the end of the scrollback clamps instead of going negative`() {
        // 钳位由生产函数 scrollOffsetForRow 完成，不在测试里重做一遍：
        // 此前本用例自己写 `staleOffset.coerceIn(...)`，把生产里的钳位删掉也判不了红。
        val scrollbackLength = 100
        // 请求滚到回滚顶部之上（行号 -50）：偏移被钳到上限 100，视口顶行因此是 0，
        // 即显示最早的一行而不是负的视口顶行号。
        val viewportTopGrid = scrollbackLength - scrollOffsetForRow(-50, scrollbackLength)
        assertEquals(0, viewportTopGrid)
        assertPoint(
            gridToScreen(row = 5, col = 2, viewportTopGrid = viewportTopGrid, cellWidth = 8f, cellHeight = 16f),
            16f,
            5 * 16f,
        )
        // 反向：请求滚到末行之下时偏移被钳到 0，视口顶行是回滚长度。
        assertEquals(100, scrollbackLength - scrollOffsetForRow(scrollbackLength + 500, scrollbackLength))
    }

    @Test
    fun `negative viewportTopGrid still converts purely without crashing`() {
        // Documented behavior: the function itself never clamps — a negative
        // top pushes content DOWN linearly (rows render below their slot).
        assertPoint(
            gridToScreen(row = 5, col = 1, viewportTopGrid = -50, cellWidth = 8f, cellHeight = 16f),
            8f,
            55 * 16f,
        )
    }

    // ── corners ───────────────────────────────────────────────────────────

    @Test
    fun `selection rectangle corners match the four mapped edges`() {
        // onGetContentRect maps (topRow,leftCol) and (bottomRow+1,rightCol+1).
        val cw = 11f
        val ch = 21f
        val vtg = 90
        val topLeft = gridToScreen(row = 95, col = 4, viewportTopGrid = vtg, cellWidth = cw, cellHeight = ch)
        val bottomRight = gridToScreen(row = 98, col = 30, viewportTopGrid = vtg, cellWidth = cw, cellHeight = ch)
        assertPoint(topLeft, 4 * cw, 5 * ch)
        assertPoint(bottomRight, 30 * cw, 8 * ch)
    }
}
