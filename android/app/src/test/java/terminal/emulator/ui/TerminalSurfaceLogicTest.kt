package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure interaction logic extracted from TerminalSurface (multi-tap selection
 * counting, edge-scroll zones, pixel→cell mapping, wide-char snapping) —
 * JVM-testable without a view, bridge, or MotionEvent.
 */
class TerminalSurfaceLogicTest {

    private val tapWindowMs = 400L

    // ── tap counting ──────────────────────────────────────────────────────────

    @Test
    fun `rapid tap increments the count`() {
        assertEquals(2, nextTapCount(now = 500, lastTapTime = 200, tapCount = 1, windowMs = tapWindowMs))
        assertEquals(3, nextTapCount(now = 700, lastTapTime = 500, tapCount = 2, windowMs = tapWindowMs))
    }

    @Test
    fun `tap at the window edge starts a fresh click`() {
        // Strict `<`: now - last == windowMs does NOT count as rapid.
        assertEquals(1, nextTapCount(now = 600, lastTapTime = 200, tapCount = 4, windowMs = tapWindowMs))
    }

    @Test
    fun `slow tap resets the count to one`() {
        assertEquals(1, nextTapCount(now = 1_000, lastTapTime = 200, tapCount = 1, windowMs = tapWindowMs))
        assertEquals(1, nextTapCount(now = 600, lastTapTime = 100, tapCount = 3, windowMs = tapWindowMs))
    }

    @Test
    fun `count grows past select all without rolling over`() {
        assertEquals(5, nextTapCount(now = 500, lastTapTime = 200, tapCount = 4, windowMs = tapWindowMs))
        assertEquals(6, nextTapCount(now = 500, lastTapTime = 200, tapCount = 5, windowMs = tapWindowMs))
    }

    // ── multi-tap action mapping ──────────────────────────────────────────────

    @Test
    fun `tap counts map to word line and select all`() {
        assertEquals(MultiTapAction.NOT_A_MULTI_TAP, multiTapAction(1))
        assertEquals(MultiTapAction.WORD, multiTapAction(2))
        assertEquals(MultiTapAction.LINE, multiTapAction(3))
        assertEquals(MultiTapAction.SELECT_ALL, multiTapAction(4))
        assertEquals(MultiTapAction.SELECT_ALL, multiTapAction(6))
        assertEquals(MultiTapAction.NOT_A_MULTI_TAP, multiTapAction(0))
    }

    // ── edge-scroll zones ─────────────────────────────────────────────────────

    @Test
    fun `top zone scrolls up and bottom zone scrolls down`() {
        val cellHeight = 20f
        val surfaceHeightPx = 200f
        // Top boundary is strict `<`: dead-center of the top half-cell is up.
        assertEquals(EdgeScrollDirection.UP, edgeScrollDirection(y = 9f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight))
        // Bottom boundary is `>= surface - half`: exactly at the edge is down.
        assertEquals(EdgeScrollDirection.DOWN, edgeScrollDirection(y = 190f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight))
        assertEquals(EdgeScrollDirection.DOWN, edgeScrollDirection(y = 200f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight))
        assertEquals(EdgeScrollDirection.STOP, edgeScrollDirection(y = 100f, surfaceHeightPx = surfaceHeightPx, cellHeight = cellHeight))
    }

    @Test
    fun `degenerate surface favors the top zone`() {
        // Surface shorter than a cell: top zone (y < cellHeight/2) wins at 0
        // when both zones would overlap — matches the `<`/`>=` asymmetry.
        assertEquals(EdgeScrollDirection.UP, edgeScrollDirection(y = 0f, surfaceHeightPx = 10f, cellHeight = 20f))
        assertEquals(EdgeScrollDirection.DOWN, edgeScrollDirection(y = 10f, surfaceHeightPx = 10f, cellHeight = 20f))
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

    // ── wide-char snapping ────────────────────────────────────────────────────

    @Test
    fun `trailing half of a wide char snaps left`() {
        // '中' is wide (2 cells) starting at col 0, so col 1 (trailing half) → 0.
        val line = "中文AB"
        assertEquals(0, snapColToWideChar(line, col = 1))
        // Second wide char occupies cols 2..3: col 3 → 2.
        assertEquals(2, snapColToWideChar(line, col = 3))
    }

    @Test
    fun `leading half and ascii columns stay put`() {
        val line = "中文AB"
        assertEquals(0, snapColToWideChar(line, col = 0))
        assertEquals(4, snapColToWideChar(line, col = 4))
        assertEquals(5, snapColToWideChar(line, col = 5))
    }

    @Test
    fun `col zero and empty line are passthrough`() {
        assertEquals(0, snapColToWideChar(line = "", col = 0))
        assertEquals(3, snapColToWideChar(line = "", col = 3))
    }
}
