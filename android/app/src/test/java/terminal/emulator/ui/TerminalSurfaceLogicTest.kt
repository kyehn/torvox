package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure interaction logic extracted from TerminalSurface (edge-scroll zones,
 * pixel→cell mapping, wide-char snapping) —
 * JVM-testable without a view, bridge, or MotionEvent.
 */
class TerminalSurfaceLogicTest {

    // ── edge-scroll zones ─────────────────────────────────────────────────────

    @Test
    fun `top zone scrolls up and bottom zone scrolls down`() {
        val cellHeight = 20f
        val surfaceHeightPx = 200f
        // Top boundary is strict `<`: dead-center of the top half-cell is up.
        assertEquals(
            EdgeScrollDirection.UP,
            edgeScrollDirection(yPx = 9f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight),
        )
        // Bottom boundary is `>= surface - half`: exactly at the edge is down.
        assertEquals(
            EdgeScrollDirection.DOWN,
            edgeScrollDirection(yPx = 190f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight),
        )
        assertEquals(
            EdgeScrollDirection.DOWN,
            edgeScrollDirection(yPx = 200f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight),
        )
        assertEquals(
            EdgeScrollDirection.STOP,
            edgeScrollDirection(yPx = 100f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight),
        )
    }

    @Test
    fun `degenerate surface favors the top zone`() {
        // Surface shorter than a cell: top zone (y < cellHeight/2) wins at 0
        // when both zones would overlap — matches the `<`/`>=` asymmetry.
        assertEquals(EdgeScrollDirection.UP, edgeScrollDirection(yPx = 0f, surfaceHeightPx = 10f, cellHeight = 20f))
        assertEquals(EdgeScrollDirection.DOWN, edgeScrollDirection(yPx = 10f, surfaceHeightPx = 10f, cellHeight = 20f))
    }

    // ── pixel → cell mapping ──────────────────────────────────────────────────

    @Test
    fun `pixel maps to floored cell`() {
        assertEquals(3, pixelToCell(px = 70f, cellSize = 20f, maxCells = 40))
        assertEquals(0, pixelToCell(px = 19f, cellSize = 20f, maxCells = 40))
        assertEquals(9, pixelToCell(px = 199f, cellSize = 20f, maxCells = 40))
    }

    @Test
    fun `pixel is clamped to the grid`() {
        assertEquals(0, pixelToCell(px = -50f, cellSize = 20f, maxCells = 40))
        assertEquals(39, pixelToCell(px = 10_000f, cellSize = 20f, maxCells = 40))
        // Zero-cell grid stays 0 instead of clamping to -1.
        assertEquals(0, pixelToCell(px = 10_000f, cellSize = 20f, maxCells = 0))
    }

    // ── wide-char cell mapping ───────────────────────────────────────────────

    @Test
    fun `isWhitespaceCell indexes by grid column`() {
        // scrollbackLine 每列恰好一个字符（原生不变式）：「中」占列 0..1，空格占列 2，
        // 列 3 起为行尾。宽字符的吸附由 cellCharStartCol 在调用前完成，此处只按列取字符。
        val line = "中 a"
        assertFalse("wide glyph lead cell is text", isWhitespaceCell(line, 0))
        assertTrue("a real space cell is blank", isWhitespaceCell(line, 1))
        assertFalse("the narrow glyph after a wide one is text", isWhitespaceCell(line, 2))
        assertTrue("past end of line is blank", isWhitespaceCell(line, 3))
        assertTrue("null line is blank", isWhitespaceCell(null, 0))
        assertTrue("a space cell is blank", isWhitespaceCell("  ", 0))
    }

    @Test
    fun `isWhitespaceCell does not drift across wide glyphs`() {
        // 旧的宽度累加模型把「宽字符之后的空格」读成前一个宽字符，判位每经一个
        // 宽字符再左移一格：中日韩行上的空白长按恒弹完整菜单而非仅粘贴。
        assertTrue("blank right after a wide glyph is still blank", isWhitespaceCell("中 a", 1))
        assertFalse("narrow glyph right after a wide glyph is text", isWhitespaceCell("中 a", 2))
        assertTrue("blank after two wide glyphs is still blank", isWhitespaceCell("中 文 a", 3))
        assertFalse("narrow glyph after two wide glyphs is text", isWhitespaceCell("中 文 a", 4))
    }

    @Test
    fun `isWhitespaceCell stays blank past the trimmed line end`() {
        // 行文本经 trim_end 变短；行尾之后的列仍算空白。
        assertTrue(isWhitespaceCell("中", 1))
        assertTrue(isWhitespaceCell("中", 40))
    }

    // ── clampSelection (order-preserving range clamp) ─────────────────────────

    @Test
    fun `clampSelection keeps an ordered in-bounds selection unchanged`() {
        assertEquals(
            SelectionBounds(startRow = 2, startCol = 3, endRow = 5, endCol = 7),
            clampSelection(2, 3, 5, 7, maxRow = 23, maxCol = 79),
        )
    }

    @Test
    fun `clampSelection swaps an inverted selection`() {
        // End dragged above/left of start → anchors swap so start ≤ end.
        assertEquals(
            SelectionBounds(startRow = 1, startCol = 4, endRow = 6, endCol = 9),
            clampSelection(6, 9, 1, 4, maxRow = 23, maxCol = 79),
        )
        // Same-row inversion (end col before start col).
        assertEquals(
            SelectionBounds(startRow = 3, startCol = 2, endRow = 3, endCol = 8),
            clampSelection(3, 8, 3, 2, maxRow = 23, maxCol = 79),
        )
    }

    @Test
    fun `clampSelection clamps out-of-bounds anchors to the grid`() {
        assertEquals(
            SelectionBounds(startRow = 0, startCol = 0, endRow = 23, endCol = 79),
            clampSelection(-5, -1, 100, 200, maxRow = 23, maxCol = 79),
        )
    }

    @Test
    fun `clampSelection clamps then swaps so the result is always ordered`() {
        // Both anchors out of bounds AND inverted: clamping alone would leave
        // start > end; the swap must run after clamping.
        assertEquals(
            SelectionBounds(startRow = 0, startCol = 0, endRow = 10, endCol = 10),
            clampSelection(50, 50, -10, -10, maxRow = 10, maxCol = 10),
        )
    }

    @Test
    fun `clampSelection degenerates on empty grids`() {
        // Negative max sizes coerce to zero-size grids: everything collapses
        // to (0,0)-(0,0) instead of negative coordinates.
        assertEquals(
            SelectionBounds(0, 0, 0, 0),
            clampSelection(4, 5, 8, 9, maxRow = -1, maxCol = -3),
        )
        assertEquals(
            SelectionBounds(0, 0, 0, 0),
            clampSelection(0, 0, 0, 0, maxRow = 10, maxCol = 10),
        )
    }

    @Test
    fun `clampSelection keeps scrolled-off selections in absolute space`() {
        // 选区行是绝对行（0 = 回滚顶部）：回滚 100 行、视口 24 行时，
        // 上界必须是 100 + 24 - 1，视口下半部分的词选区不得被拉到视口最后一行。
        val absoluteMaxRow = 100 + 24 - 1
        assertEquals(
            SelectionBounds(startRow = 110, startCol = 3, endRow = 110, endCol = 9),
            clampSelection(110, 3, 110, 9, maxRow = absoluteMaxRow, maxCol = 79),
        )
    }

    // ── 300ms menu re-show guard ─────────────────────────────────────────────

    @Test
    fun `tap inside the guard window is suppressed after a drag ends`() {
        // Release instant and just under the window boundary are suppressed.
        assert(shouldSuppressTapAfterDragEnd(nowMs = 1_000, lastDragEndMs = 1_000))
        assert(shouldSuppressTapAfterDragEnd(nowMs = 1_299, lastDragEndMs = 1_000))
    }

    @Test
    fun `tap at or past the guard boundary is a real tap`() {
        // Strict `<` at the boundary: exactly 300ms after release is NOT
        // suppressed.
        assert(!shouldSuppressTapAfterDragEnd(nowMs = 1_300, lastDragEndMs = 1_000))
        assert(!shouldSuppressTapAfterDragEnd(nowMs = 2_500, lastDragEndMs = 1_000))
    }

    @Test
    fun `no prior drag end means the guard is inactive`() {
        assert(!shouldSuppressTapAfterDragEnd(nowMs = 100, lastDragEndMs = 0L))
    }

    // ── pinch zoom mapping ───────────────────────────────────────────────────

    @Test
    fun `zoom scales around the gesture base size`() {
        assertEquals(25f, zoomFontSize(20f, 1.25f))
        assertEquals(15f, zoomFontSize(20f, 0.75f))
    }

    @Test
    fun `zoom clamps to the same bounds as the settings screen`() {
        assertEquals(48f, zoomFontSize(16f, 5f))
        assertEquals(14f, zoomFontSize(16f, 0.1f))
    }

    @Test
    fun `pinch sequence accumulates then settles on a new size`() {
        // Begin(16sp) → previews → end: cumulative factor decides one outcome.
        var factor = 1.0f
        factor *= 1.1f
        assertEquals(17.6f, zoomFontSize(16f, factor))
        factor *= 1.1f
        val finalSize = zoomFontSize(16f, factor)
        assert(zoomSettledOnNewSize(16f, finalSize))
    }

    @Test
    fun `pinch returning to base only reverts the preview`() {
        // Tiny drift under epsilon: no persist, just revert to the base size.
        val finalSize = zoomFontSize(16f, 1.001f)
        assert(!zoomSettledOnNewSize(16f, finalSize))
    }

    @Test
    fun `no-op pinch on a below-zoom-min base does not settle`() {
        // N2-96: settings allows 8sp but zoom clamps to 14sp; onScaleEnd must judge
        // the unclamped product so a net-1.0 gesture never persists a new size.
        val rawSizeSp = 8f * 1.0f
        assert(!zoomSettledOnNewSize(8f, rawSizeSp))
    }

    // ── menu anchoring (design decision 3) ────────────────────────────────────

    private val viewport = PixelRect(0, 0, 400, 800)

    @Test
    fun `menu anchors above the selection, centered`() {
        // 上方优先：底缘高出选择顶缘一个手柄高；水平居中 (100+200)/2-180/2=60。
        val anchor = menuAnchor(
            selection = PixelRect(100, 300, 200, 320),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(60 to 216, anchor)
    }

    @Test
    fun `menu flips below when the selection touches the top`() {
        // 上方贴顶（20-44-40<0）→ 翻到选择底缘之下 (60+40)；x 居中 90-90=0。
        val anchor = menuAnchor(
            selection = PixelRect(40, 20, 140, 60),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(0 to 100, anchor)
    }

    @Test
    fun `menu clamps to the right edge`() {
        // 中心 385 → 295 超出右钳制 400-180=220，贴右夹回。
        val anchor = menuAnchor(
            selection = PixelRect(350, 300, 420, 320),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(220 to 216, anchor)
    }

    @Test
    fun `menu hides when the selection covers the viewport`() {
        // 全选盖满视口：上方越顶、下方越底 → 无处可放，返回 null（隐藏）。
        val anchor = menuAnchor(
            selection = PixelRect(0, 0, 400, 800),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(null, anchor)
    }

    @Test
    fun `menu hides when the viewport has no size`() {
        val anchor = menuAnchor(
            selection = PixelRect(10, 10, 20, 20),
            viewport = PixelRect(0, 0, 0, 0),
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(null, anchor)
    }

    @Test
    fun `menu hides when the selection is scrolled out of the viewport`() {
        // 选区整体在视口之上（翻阅回滚后选中区仍留在旧处）：上方落点越顶、下方落点
        // 虽然「不越底」却在视口外，两侧都不可用 → 隐藏，不返回屏外锚点。
        val anchor = menuAnchor(
            selection = PixelRect(0, -400, 400, -200),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(null, anchor)
    }

    @Test
    fun `menu hides when the selection is below the viewport`() {
        // 选区整体在视口之下：上方落点不越顶却落在视口外，同样隐藏。
        val anchor = menuAnchor(
            selection = PixelRect(0, 900, 400, 1000),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(null, anchor)
    }

    @Test
    fun `menu stays inside the viewport for a selection clipped at the bottom`() {
        // 选区下缘越过视口底：上方落点整体在视口内（600-44-40=516，516+44=560 ≤ 800）。
        val anchor = menuAnchor(
            selection = PixelRect(0, 600, 400, 900),
            viewport = viewport,
            menuWidth = 180,
            menuHeight = 44,
            handleHeight = 40,
        )
        assertEquals(110 to 516, anchor)
    }
}
