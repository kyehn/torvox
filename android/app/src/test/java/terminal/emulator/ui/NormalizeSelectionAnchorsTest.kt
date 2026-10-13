package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 选区端点归一化：上游保留选择方向，`start`/`end` 可反向，按「先比行、行同再比列」
 * 归一为 (起点, 终点)。
 *
 * 这段逻辑曾有两份且已分叉——菜单锚定那份漏了「行相等」分支，对向左下的选区算出
 * leftCol > rightCol；取文那份是对的。菜单只取 `(left + right) / 2` 故当时不可见。
 */
class NormalizeSelectionAnchorsTest {

    private data class Anchor(val row: Int, val col: Int)

    private fun normalize(start: Anchor, end: Anchor): Pair<Anchor, Anchor> =
        normalizeSelectionAnchors(start, end, { it.row }, { it.col })

    @Test
    fun `forward selection is unchanged`() {
        val start = Anchor(3, 2)
        val end = Anchor(3, 8)
        assertEquals(start to end, normalize(start, end))
    }

    @Test
    fun `reverse selection on one row is swapped`() {
        val start = Anchor(3, 8)
        val end = Anchor(3, 2)
        assertEquals(Anchor(3, 2) to Anchor(3, 8), normalize(start, end))
    }

    @Test
    fun `multi-row selection ignores columns and orders by row`() {
        val start = Anchor(7, 1)
        val end = Anchor(2, 9)
        assertEquals(Anchor(2, 9) to Anchor(7, 1), normalize(start, end))
    }

    /**
     * 回归钉：向左下的选区（行递增、列递减）按行归一——行的先后由 `row < row` 决定，
     * 不看列。多行选区的列是各行内偏移，本就没有「全局左右」，故列序不在此断言内
     * （菜单锚定那侧另取 min/max 作包围盒）。
     */
    @Test
    fun `down-left selection orders by row not column`() {
        val start = Anchor(2, 3)
        val end = Anchor(5, 1)
        val (lo, hi) = normalize(start, end)
        assertEquals(lo.row to lo.col, 2 to 3)
        assertEquals(hi.row to hi.col, 5 to 1)
        assertEquals("行序不得反向", true, lo.row <= hi.row)
    }

    /** 同一行内的反向选择才按列换向。 */
    @Test
    fun `same-row reverse selection swaps by column`() {
        val start = Anchor(3, 8)
        val end = Anchor(3, 2)
        val (lo, hi) = normalize(start, end)
        assertEquals(lo.col <= hi.col, true)
    }

    @Test
    fun `equal endpoints are stable`() {
        val anchor = Anchor(4, 4)
        assertEquals(anchor to anchor, normalize(anchor, anchor))
    }
}
