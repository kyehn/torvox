package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure surface×cell → rows/cols computation behind
 * TerminalRuntime.recomputeGridFromFontMetrics: fractional cells floor,
 * tiny surfaces clamp to one cell, degenerate input returns the (0, 0)
 * sentinel, and huge surfaces stay inside Int range.
 */
class ComputeGridDimensionsTest {

    // ── typical geometry ──────────────────────────────────────────────────

    @Test
    fun `typical phone geometry yields exact grid`() {
        // 1080x2400 px surface, 30x60 px cells (density-scaled).
        val (rows, cols) = computeGridDimensions(surfaceWidth = 1080, surfaceHeight = 2400, cellWidth = 30f, cellHeight = 60f)
        assertEquals(36, cols)
        assertEquals(40, rows)
    }

    @Test
    fun `fractional leftover cells floor down`() {
        // 100 / 30 = 3.33 → 3; 199 / 50 = 3.98 → 3.
        val (rows, cols) = computeGridDimensions(surfaceWidth = 100, surfaceHeight = 199, cellWidth = 30f, cellHeight = 50f)
        assertEquals(3, cols)
        assertEquals(3, rows)
    }

    @Test
    fun `exact multiples waste no row or column`() {
        val (rows, cols) = computeGridDimensions(surfaceWidth = 600, surfaceHeight = 900, cellWidth = 20f, cellHeight = 45f)
        assertEquals(30, cols)
        assertEquals(20, rows)
    }

    @Test
    fun `modifier-bar subtraction happens in the caller`() {
        // The pure function sees the already-subtracted usable height:
        // 2400px surface − 200px bar → 2200 usable → 22 rows of 100px.
        val (rows, _) =
            computeGridDimensions(
                surfaceWidth = 1080,
                surfaceHeight = 2400 - 200,
                cellWidth = 30f,
                cellHeight = 100f,
            )
        assertEquals(22, rows)
    }

    // ── extreme surfaces ──────────────────────────────────────────────────

    @Test
    fun `tiny surface clamps to one cell per axis`() {
        val (rows, cols) = computeGridDimensions(surfaceWidth = 1, surfaceHeight = 1, cellWidth = 30f, cellHeight = 60f)
        assertEquals(1, cols)
        assertEquals(1, rows)
    }

    @Test
    fun `surface smaller than one cell still yields one cell`() {
        val (rows, cols) = computeGridDimensions(surfaceWidth = 29, surfaceHeight = 59, cellWidth = 30f, cellHeight = 60f)
        assertEquals(1, cols)
        assertEquals(1, rows)
    }

    @Test
    fun `huge surface stays within int range without overflow`() {
        val (rows, cols) =
            computeGridDimensions(
                surfaceWidth = Int.MAX_VALUE,
                surfaceHeight = Int.MAX_VALUE,
                cellWidth = 128f,
                cellHeight = 256f,
            )
        // Float semantics: Int.MAX_VALUE widens to exactly 2^31 in the
        // Int→Float conversion, so the quotients are exact powers of two
        // (one more than integer division of 2147483647 would give).
        assertEquals(16_777_216, cols)
        assertEquals(8_388_608, rows)
        assertTrue(cols > 0 && rows > 0)
    }

    // ── zero-value clamping / sentinel ────────────────────────────────────

    @Test
    fun `zero surface returns the degenerate sentinel`() {
        assertEquals(Pair(0, 0), computeGridDimensions(0, 2400, 30f, 60f))
        assertEquals(Pair(0, 0), computeGridDimensions(1080, 0, 30f, 60f))
    }

    @Test
    fun `zero or negative cell metrics return the degenerate sentinel`() {
        assertEquals(Pair(0, 0), computeGridDimensions(1080, 2400, 0f, 60f))
        assertEquals(Pair(0, 0), computeGridDimensions(1080, 2400, 30f, 0f))
        assertEquals(Pair(0, 0), computeGridDimensions(1080, 2400, -30f, 60f))
        assertEquals(Pair(0, 0), computeGridDimensions(1080, 2400, 30f, -60f))
    }

    @Test
    fun `negative surfaces return the degenerate sentinel`() {
        assertEquals(Pair(0, 0), computeGridDimensions(-1080, 2400, 30f, 60f))
        assertEquals(Pair(0, 0), computeGridDimensions(1080, -2400, 30f, 60f))
    }

    @Test
    fun `sentinel is distinguishable from a real one-cell grid`() {
        // A 1x1 px surface is a legitimate (if useless) grid: rows/cols = 1.
        val realOneCell = computeGridDimensions(1, 1, 30f, 60f)
        val degenerate = computeGridDimensions(0, 0, 30f, 60f)
        assertEquals(Pair(1, 1), realOneCell)
        assertEquals(Pair(0, 0), degenerate)
        assertTrue(degenerate.first != realOneCell.first || degenerate.second != realOneCell.second)
    }
}
