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

    private fun assertPoint(
        actual: Pair<Float, Float>,
        expectedX: Float,
        expectedY: Float,
    ) {
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
        // scrollbackLength=1000, 24 visible rows, scrolled fully back:
        // offset = 1000 - 24 ⇒ viewportTopGrid = 1000 - offset = 24.
        val scrollbackLength = 1000
        val visibleRows = 24
        val offset = scrollbackLength - visibleRows
        val viewportTopGrid = scrollbackLength - offset
        assertEquals(visibleRows, viewportTopGrid)
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
        // scrollToRow(hitRow): targetOffset = (len - row).coerceIn(0, len)
        // ⇒ offset = len - hitRow ⇒ viewportTopGrid = hitRow ⇒ y = 0.
        val scrollbackLength = 5000
        val hitRow = 4321
        val offset = (scrollbackLength - hitRow).coerceIn(0, scrollbackLength)
        val viewportTopGrid = scrollbackLength - offset
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
        val row = 130
        val col = 41
        val viewportTopGrid = 118 // fixed scroll position while zooming
        // Small → large font: same grid point, proportionally larger pixels.
        for (cell in listOf(6f to 12f, 10f to 20f, 14f to 28f, 24f to 48f)) {
            val (cw, ch) = cell
            val p = gridToScreen(row, col, viewportTopGrid, cw, ch)
            assertEquals(col * cw, p.first, epsilon)
            assertEquals((row - viewportTopGrid) * ch, p.second, epsilon)
        }
    }

    @Test
    fun `scroll x font-size combination stays consistent with hand math`() {
        // Cross product of scroll states and font sizes: the conversion must
        // stay a pure affine map in both axes.
        for (viewportTopGrid in listOf(0, 7, 250)) {
            for (cell in listOf(8f to 16f, 13f to 26f)) {
                val (cw, ch) = cell
                val expectedX = 12 * cw
                val expectedY = (31 - viewportTopGrid) * ch
                assertPoint(gridToScreen(31, 12, viewportTopGrid, cw, ch), expectedX, expectedY)
            }
        }
    }

    // ── negative-offset clamping ──────────────────────────────────────────

    @Test
    fun `negative scroll offset is clamped before conversion`() {
        // A stale offset > scrollbackLength would make viewportTopGrid
        // negative; callers clamp it (same coercion family as scrollToRow's
        // coerceIn). After clamping the conversion is the zero-scroll case.
        val scrollbackLength = 100
        val staleOffset = 150
        val clampedOffset = staleOffset.coerceIn(0, scrollbackLength)
        val viewportTopGrid = (scrollbackLength - clampedOffset).coerceAtLeast(0)
        assertEquals(0, viewportTopGrid)
        assertPoint(
            gridToScreen(row = 5, col = 2, viewportTopGrid = viewportTopGrid, cellWidth = 8f, cellHeight = 16f),
            16f,
            80f,
        )
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
