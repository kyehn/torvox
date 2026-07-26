package io.term.selection

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.roundToInt

class SelectionCoordinateTest {
    companion object {
        private const val GRID_ROWS = 24
        private const val GRID_COLS = 80
        private const val CELL_WIDTH = 12f
        private const val CELL_HEIGHT = 20f
        private const val SURFACE_WIDTH = GRID_COLS * CELL_WIDTH.toInt()
        private const val SURFACE_HEIGHT = GRID_ROWS * CELL_HEIGHT.toInt()
        private const val HANDLE_WIDTH = 24
        private const val ORIENTATION_LEFT = 0
        private const val ORIENTATION_RIGHT = 1
    }

    @Test
    fun pixelToGrid_mapsOriginToZeroZero() {
        val (row, col) = pixelToGrid(0f, 0f, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(0, row)
        assertEquals(0, col)
    }

    @Test
    fun pixelToGrid_mapsCellCenterCorrectly() {
        val px = 5 * CELL_WIDTH + CELL_WIDTH / 2f
        val py = 10 * CELL_HEIGHT + CELL_HEIGHT / 2f
        val (row, col) = pixelToGrid(px, py, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(10, row)
        assertEquals(5, col)
    }

    @Test
    fun pixelToGrid_mapsCellBoundaryToFloorRow() {
        val px = 3 * CELL_WIDTH
        val py = 7 * CELL_HEIGHT
        val (row, col) = pixelToGrid(px, py, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(7, row)
        assertEquals(3, col)
    }

    @Test
    fun pixelToGrid_mapsNearCellEdgeCorrectly() {
        val px = 15 * CELL_WIDTH - 1f
        val py = 20 * CELL_HEIGHT - 1f
        val (row, col) = pixelToGrid(px, py, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(19, row)
        assertEquals(14, col)
    }

    @Test
    fun pixelToGrid_clampsNegativeCoordinates() {
        val (row, col) = pixelToGrid(-50f, -30f, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(0, row)
        assertEquals(0, col)
    }

    @Test
    fun pixelToGrid_clampsBeyondGrid() {
        val (row, col) =
            pixelToGrid(
                (GRID_COLS * CELL_WIDTH + 100f),
                (GRID_ROWS * CELL_HEIGHT + 100f),
                CELL_WIDTH,
                CELL_HEIGHT,
                GRID_ROWS,
                GRID_COLS,
            )
        assertEquals(GRID_ROWS - 1, row)
        assertEquals(GRID_COLS - 1, col)
    }

    @Test
    fun gridToPixel_topLeftCellOrigin() {
        val (x, y) = gridToPixel(0, 0, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(0f, x, 0.001f)
        assertEquals(0f, y, 0.001f)
    }

    @Test
    fun gridToPixel_midGridCellOrigin() {
        val (x, y) = gridToPixel(12, 40, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(40f * CELL_WIDTH, x, 0.001f)
        assertEquals(12f * CELL_HEIGHT, y, 0.001f)
    }

    @Test
    fun gridToPixel_endCursorPosition() {
        val (endX, endY) = gridToEndPixel(5, 10, CELL_WIDTH, CELL_HEIGHT, 0)
        assertEquals((10 + 1) * CELL_WIDTH, endX, 0.001f)
        assertEquals((5 + 1) * CELL_HEIGHT, endY, 0.001f)
    }

    @Test
    fun gridToPixel_takesScrollOffsetIntoAccount() {
        val scrollOffset = 5
        val visibleRow = (10 - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        val (_, y) = gridToPixel(visibleRow, 3, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(10 - scrollOffset, (y / CELL_HEIGHT).roundToInt())
    }

    @Test
    fun handleAnchor_startHandleLeftSide() {
        val col = 5
        val cursorX = (col * CELL_WIDTH).roundToInt()
        val handleWidth = HANDLE_WIDTH
        val surfaceLeft = 0
        val surfaceWidth = SURFACE_WIDTH

        val orientation =
            when {
                cursorX - handleWidth < 0 -> ORIENTATION_RIGHT
                cursorX + handleWidth > surfaceWidth -> ORIENTATION_LEFT
                else -> ORIENTATION_LEFT
            }
        val hotspotX =
            if (orientation == ORIENTATION_LEFT) (handleWidth * 3) / 4 else handleWidth / 4
        val popupX =
            (surfaceLeft + cursorX - hotspotX)
                .coerceIn(surfaceLeft, (surfaceLeft + surfaceWidth - handleWidth))

        assertEquals(ORIENTATION_LEFT, orientation)
        assertEquals((handleWidth * 3) / 4, hotspotX)
        assertTrue("popupX $popupX should be >= $surfaceLeft", popupX >= surfaceLeft)
    }

    @Test
    fun handleAnchor_startHandleNearLeftEdge_shiftsToRight() {
        val col = 0
        val cursorX = (col * CELL_WIDTH).roundToInt()
        val handleWidth = HANDLE_WIDTH
        val surfaceLeft = 0
        val surfaceWidth = SURFACE_WIDTH

        val orientation =
            when {
                cursorX - handleWidth < 0 -> ORIENTATION_RIGHT
                cursorX + handleWidth > surfaceWidth -> ORIENTATION_LEFT
                else -> ORIENTATION_LEFT
            }
        assertEquals(ORIENTATION_RIGHT, orientation)
    }

    @Test
    fun handleAnchor_endHandleRightSide() {
        val col = 75
        val cursorX = ((col + 1) * CELL_WIDTH).roundToInt()
        val handleWidth = HANDLE_WIDTH
        val surfaceLeft = 0
        val surfaceWidth = SURFACE_WIDTH

        val orientation =
            when {
                cursorX + handleWidth > surfaceWidth -> ORIENTATION_LEFT
                cursorX - handleWidth < 0 -> ORIENTATION_RIGHT
                else -> ORIENTATION_RIGHT
            }
        val hotspotX =
            if (orientation == ORIENTATION_LEFT) (handleWidth * 3) / 4 else handleWidth / 4
        val popupX =
            (surfaceLeft + cursorX - hotspotX)
                .coerceIn(surfaceLeft, (surfaceLeft + surfaceWidth - handleWidth))

        assertEquals(ORIENTATION_RIGHT, orientation)
        assertEquals(handleWidth / 4, hotspotX)
        assertTrue("popupX $popupX should fit within surface", popupX + handleWidth <= surfaceLeft + surfaceWidth)
    }

    @Test
    fun handleAnchor_endHandleNearRightEdge_shiftsToLeft() {
        val col = GRID_COLS - 1
        val cursorX = ((col + 1) * CELL_WIDTH).roundToInt()
        val handleWidth = HANDLE_WIDTH
        val surfaceWidth = SURFACE_WIDTH

        val orientation =
            when {
                cursorX + handleWidth > surfaceWidth -> ORIENTATION_LEFT
                cursorX - handleWidth < 0 -> ORIENTATION_RIGHT
                else -> ORIENTATION_RIGHT
            }
        assertEquals(ORIENTATION_LEFT, orientation)
    }

    @Test
    fun toolbarContentRect_boundariesWithinSurface() {
        val selStartRow = 10
        val toolbarHeight = 40
        val toolbarY = maxOf(0, (selStartRow * CELL_HEIGHT).roundToInt() - toolbarHeight)
        assertTrue("Toolbar should be above selection", selStartRow * CELL_HEIGHT.roundToInt() > toolbarY)
        assertTrue("Toolbar should not go off top", toolbarY >= 0)

        val toolbarX = 0
        val toolbarWidth = GRID_COLS * CELL_WIDTH.roundToInt()
        assertEquals(0, toolbarX)
        assertEquals(SURFACE_WIDTH, toolbarWidth)
    }

    @Test
    fun toolbarContentRect_limitedByScreenTop() {
        val selStartRow = 1
        val toolbarHeight = 40
        val toolbarY = maxOf(0, (selStartRow * CELL_HEIGHT).roundToInt() - toolbarHeight)
        assertEquals(0, toolbarY)
    }

    @Test
    fun edgeScroll_computeOffsetNearTop() {
        val touchY = 5f
        val scrollThreshold = 30f
        val scrollDelta =
            if (touchY < scrollThreshold) {
                ((scrollThreshold - touchY) / 3f).roundToInt()
            } else {
                0
            }
        assertTrue("Should scroll positively near top", scrollDelta > 0)
    }

    @Test
    fun edgeScroll_noScrollInMiddle() {
        val touchY = CELL_HEIGHT * 10f
        val scrollThreshold = 30f
        val surfaceHeight = SURFACE_HEIGHT
        val scrollDelta =
            when {
                touchY < scrollThreshold -> ((scrollThreshold - touchY) / 3f).roundToInt()
                touchY > surfaceHeight - scrollThreshold -> -((touchY - (surfaceHeight - scrollThreshold)) / 3f).roundToInt()
                else -> 0
            }
        assertEquals(0, scrollDelta)
    }

    @Test
    fun edgeScroll_computeOffsetNearBottom() {
        val touchY = (SURFACE_HEIGHT - 5f)
        val scrollThreshold = 30f
        val scrollDelta =
            when {
                touchY > SURFACE_HEIGHT - scrollThreshold -> -((touchY - (SURFACE_HEIGHT - scrollThreshold)) / 3f).roundToInt()
                else -> 0
            }
        assertTrue("Should scroll negatively near bottom", scrollDelta < 0)
    }

    @Test
    fun handleReposition_keepsStartAnchorOnGrid() {
        val newRow = 8
        val newCol = 15
        val clampedRow = newRow.coerceIn(0, GRID_ROWS - 1)
        val clampedCol = newCol.coerceIn(0, GRID_COLS - 1)
        assertEquals(8, clampedRow)
        assertEquals(15, clampedCol)
    }

    @Test
    fun handleReposition_clampsToGridBounds() {
        val newRow = -5
        val newCol = GRID_COLS + 10
        val clampedRow = newRow.coerceIn(0, GRID_ROWS - 1)
        val clampedCol = newCol.coerceIn(0, GRID_COLS - 1)
        assertEquals(0, clampedRow)
        assertEquals(GRID_COLS - 1, clampedCol)
    }

    @Test
    fun visibleRow_adjustsForScrollOffset() {
        val scrollOffset = 10
        val row = 15
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        assertEquals(5, visibleRow)
    }

    @Test
    fun visibleRow_clampsBelowZero() {
        val scrollOffset = 10
        val row = 3
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        assertEquals(0, visibleRow)
    }

    @Test
    fun visibleRow_clampsAboveGrid() {
        val scrollOffset = 0
        val row = 50
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        assertEquals(GRID_ROWS - 1, visibleRow)
    }

    @Test
    fun endCursorPosition_afterLastChar() {
        val col = 7
        val endX = ((col + 1) * CELL_WIDTH).roundToInt()
        assertEquals(8 * CELL_WIDTH.roundToInt(), endX)
    }

    @Test
    fun pixelToCursorY_withScroll() {
        val row = 12
        val scrollOffset = 3
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        val cursorY =
            (visibleRow * CELL_HEIGHT)
                .roundToInt()
                .coerceIn(0, (SURFACE_HEIGHT - 0).coerceAtLeast(0))
        assertEquals(9, visibleRow)
        assertTrue(cursorY >= 0)
    }

    @Test
    fun pixelToCursorY_endHandle() {
        val row = 10
        val scrollOffset = 2
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        val cursorY =
            ((visibleRow + 1) * CELL_HEIGHT)
                .roundToInt()
                .coerceIn(0, (SURFACE_HEIGHT - HANDLE_WIDTH).coerceAtLeast(0))
        assertEquals(8, visibleRow)
        assertTrue(cursorY >= 0)
    }

    @Test
    fun roundTrip_pixelToGridToPixel() {
        val originalCol = 42
        val originalRow = 15
        val (px, py) = gridToPixel(originalRow, originalCol, CELL_WIDTH, CELL_HEIGHT)
        val (row, col) = pixelToGrid(px + CELL_WIDTH / 2f, py + CELL_HEIGHT / 2f, CELL_WIDTH, CELL_HEIGHT)
        assertEquals(originalRow, row)
        assertEquals(originalCol, col)
    }

    @Test
    fun handleHotspotCalculation_startHandle() {
        val cursorX = 10 * CELL_WIDTH.roundToInt()
        val handleWidth = HANDLE_WIDTH
        val surfaceWidth = SURFACE_WIDTH
        val orientation = ORIENTATION_LEFT
        val hotspotX =
            if (orientation == ORIENTATION_LEFT) (handleWidth * 3) / 4 else handleWidth / 4
        val surfaceLeft = 0
        val popupX =
            (surfaceLeft + cursorX - hotspotX)
                .coerceIn(surfaceLeft, (surfaceLeft + surfaceWidth - handleWidth))
        assertEquals((handleWidth * 3) / 4, hotspotX)
        assertTrue(popupX >= surfaceLeft)
    }

    @Test
    fun handleHotspotCalculation_endHandle() {
        val cursorX = ((30 + 1) * CELL_WIDTH).roundToInt()
        val handleWidth = HANDLE_WIDTH
        val surfaceWidth = SURFACE_WIDTH
        val orientation = ORIENTATION_RIGHT
        val hotspotX =
            if (orientation == ORIENTATION_LEFT) (handleWidth * 3) / 4 else handleWidth / 4
        assertEquals(handleWidth / 4, hotspotX)
    }

    @Test
    fun surfaceDimensions_matchGridTimesCellSize() {
        assertEquals(GRID_COLS * CELL_WIDTH.toInt(), SURFACE_WIDTH)
        assertEquals(GRID_ROWS * CELL_HEIGHT.toInt(), SURFACE_HEIGHT)
    }

    @Test
    fun cellCount_derivedFromSurfaceDimensions() {
        val computedCols = SURFACE_WIDTH / CELL_WIDTH.toInt()
        val computedRows = SURFACE_HEIGHT / CELL_HEIGHT.toInt()
        assertEquals(GRID_COLS, computedCols)
        assertEquals(GRID_ROWS, computedRows)
    }

    @Test
    fun startHandleYPosition_matchesVisibleRow() {
        val row = 7
        val scrollOffset = 0
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        val cursorY = (visibleRow * CELL_HEIGHT).roundToInt()
        assertEquals((7 * CELL_HEIGHT).roundToInt(), cursorY)
    }

    @Test
    fun endHandleYPosition_belowEndRow() {
        val row = 7
        val scrollOffset = 0
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        val cursorY = ((visibleRow + 1) * CELL_HEIGHT).roundToInt()
        assertEquals((8 * CELL_HEIGHT).roundToInt(), cursorY)
    }

    @Test
    fun pixelToGrid_densityIndependent() {
        val cellW = 14f
        val cellH = 24f
        val px = 3 * cellW + cellW / 2f
        val py = 7 * cellH + cellH / 2f
        val (row, col) = pixelToGrid(px, py, cellW, cellH)
        assertEquals(7, row)
        assertEquals(3, col)
    }

    private fun pixelToGrid(
        px: Float,
        py: Float,
        cellWidth: Float,
        cellHeight: Float,
        gridRows: Int = GRID_ROWS,
        gridCols: Int = GRID_COLS,
    ): Pair<Int, Int> {
        val col = (px / cellWidth).toInt().coerceIn(0, gridCols - 1)
        val row = (py / cellHeight).toInt().coerceIn(0, gridRows - 1)
        return row to col
    }

    private fun gridToPixel(
        row: Int,
        col: Int,
        cellWidth: Float,
        cellHeight: Float,
    ): Pair<Float, Float> = (col * cellWidth) to (row * cellHeight)

    private fun gridToEndPixel(
        row: Int,
        col: Int,
        cellWidth: Float,
        cellHeight: Float,
        scrollOffset: Int,
    ): Pair<Float, Float> {
        val visibleRow = (row - scrollOffset).coerceIn(0, GRID_ROWS - 1)
        return ((col + 1) * cellWidth) to ((visibleRow + 1) * cellHeight)
    }
}
