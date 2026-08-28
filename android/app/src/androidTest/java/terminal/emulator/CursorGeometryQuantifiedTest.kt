package terminal.emulator

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.bridge.Bridge

/**
 * Quantified verification of "输入指针错误偏移 / 变方块" — the two failure modes of cursor rendering, measured
 * through real rendered pixels instead of code reading:
 *
 * METHOD — three-capture linear solve. With the cursor parked at grid (r,c) on an otherwise EMPTY
 * screen:
 * - D_col = diff(capture@col c, capture@col c+k) → width = k*cw + w
 * - D_row = diff(capture@row r, capture@row r+m) → height = m*ch + h Solving yields the cursor quad
 *   size (w,h) WITHOUT theme-color knowledge; D_row's top edge is the quad's absolute Y inside the
 *   surface (offset).
 *
 * ASSERTIONS (per DECSCUSR style: 2=block, 4=underline, 6=bar):
 * - NOT-a-giant-block ("变方块"): block/bar height <= 0.95 * cellHeight (the reported bug was a
 *   full-cell rectangle around a small glyph); underline height <= 0.30 * cellHeight.
 * - No upward offset ("偏移"): quad top >= its own row top (bearing keeps the glyph box INSIDE the
 *   cell vertically); quad bottom <= next row top.
 * - Wide-char coverage: repeating the solve one column after a printed CJK char must give width >=
 *   1.7x the ASCII width (Bar/Underline/ Block all span cell_span cells).
 * - Determinism: re-solving the same configuration twice agrees within 2 px on w/h.
 */
class CursorGeometryQuantifiedTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    private companion object {
        const val ROW = 6 // viewport row used for the probe (0-based)
        const val COL = 10
        const val DCOL = 4 // column step between paired captures
        const val DROW = 2 // row step between paired captures
    }

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** Run a shell printf that positions the cursor; wait for the frame. */
    private fun parkCursor(
        row: Int,
        col: Int,
    ) {
        // DECSCUSR-free pure CUP: 1-based row/col on the live screen.
        bridge().writeToPty("printf '\\033[$((row + 1));$((col + 1))H'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(700)
    }

    private fun applyStyle(decscusr: Int) {
        bridge().writeToPty("printf '\\033[$decscusr q'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(500)
    }

    private fun clearScreen() {
        bridge().writeToPty("clear\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(900)
    }

    /**
     * Solve the cursor quad (width,height,topY-in-surface) at [row]/[col] using the three-capture
     * method from the class doc.
     */
    private fun solveQuad(
        row: Int,
        col: Int,
    ): Triple<Int, Int, Int> {
        val dCol =
            UxTestUtils.changedBoundingBox(
                captureAt(row, col),
                captureAt(row, col + DCOL),
            )
        assertNotNull("no pixel difference between cursor columns", dCol)
        val dRow =
            UxTestUtils.changedBoundingBox(
                captureAt(row, col),
                captureAt(row + DROW, col),
            )
        assertNotNull("no pixel difference between cursor rows", dRow)
        val cw = bridge().getCellWidth()
        val ch = bridge().getCellHeight()
        val width = dCol!!.width - (DCOL * cw).toInt()
        val height = dRow!!.height - (DROW * ch).toInt()
        return Triple(width, height, dRow.top)
    }

    private fun captureAt(
        row: Int,
        col: Int,
    ): android.graphics.Bitmap {
        parkCursor(row, col)
        return UxTestUtils.screenshot(device)
    }

    @Test
    fun cursor_quad_geometry_per_style_is_glyph_box_not_full_cell() {
        val b = bridge()
        clearScreen()
        val cw = b.getCellWidth()
        val ch = b.getCellHeight()
        assertTrue("cell metrics unavailable", cw > 0f && ch > 0f)

        data class Expectation(val name: String, val decscusr: Int, val maxHeightFraction: Float)
        val styles =
            listOf(
                Expectation("block", 2, 0.95f),
                Expectation("bar", 6, 0.95f),
                Expectation("underline", 4, 0.30f),
            )

        val surfaceView = findTerminalSurface(composeTestRule.activity)
        val surfaceLoc = IntArray(2)
        surfaceView.getLocationOnScreen(surfaceLoc)

        for (style in styles) {
            applyStyle(style.decscusr)
            val (w, h, topInSurface) = solveQuad(ROW, COL)

            UxTestUtils.metric("cursor_${style.name}_w_px", w)
            UxTestUtils.metric("cursor_${style.name}_h_px", h)

            assertTrue(
                "${style.name}: quad height ${h}px is ${"%.2f".format(h / ch)}x the cell — the 'giant square' bug",
                h <= style.maxHeightFraction * ch,
            )
            assertTrue("${style.name}: degenerate quad width $w", w >= 2)

            // Offset: the quad must sit inside its own cell vertically.
            val rowTopOnScreen = surfaceLoc[1] + (ROW * ch).toInt()
            val quadTopAbsolute = surfaceLoc[1] + topInSurface
            assertTrue(
                "${style.name}: quad starts ABOVE its cell row (offset bug): top=$quadTopAbsolute rowTop=$rowTopOnScreen",
                quadTopAbsolute >= rowTopOnScreen - 2,
            )
            assertTrue(
                "${style.name}: quad extends past its cell row: bottom=${quadTopAbsolute + h}",
                quadTopAbsolute + h <= rowTopOnScreen + ch + 2,
            )
        }
    }

    @Test
    fun cursor_quad_spans_wide_characters() {
        val b = bridge()
        clearScreen()
        // Print a wide CJK char at COL, so parking the cursor on COL+1 puts
        // it visually on the trailing half of the wide glyph.
        b.writeToPty("printf '\\033[$((ROW + 1));$((COL + 1))H漢'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(900)

        applyStyle(6) // steady bar — span scaling is most visible here
        clearScreen()
        val (asciiW, _, _) = solveQuad(ROW + 4, COL)
        b.writeToPty("printf '\\033[$((ROW + 1));$((COL + 1))H漢'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(900)
        val (wideW, _, _) = solveQuad(ROW, COL + 1)

        assertTrue("baseline ASCII quad too narrow: ${asciiW}px", asciiW >= 2)
        UxTestUtils.metric("cursor_bar_ascii_w", asciiW)
        UxTestUtils.metric("cursor_bar_wide_w", wideW)
        assertTrue(
            "wide-char bar width ${wideW}px should be ~2x ascii ${asciiW}px (span ignored)",
            wideW >= asciiW * 17 / 10,
        )
    }

    @Test
    fun cursor_quad_measurement_is_deterministic() {
        clearScreen()
        applyStyle(2)
        val first = solveQuad(ROW + 8, COL + 2)
        val second = solveQuad(ROW + 8, COL + 2)
        assertTrue(
            "non-deterministic measurement: $first vs $second",
            kotlin.math.abs(first.first - second.first) <= 2 &&
                kotlin.math.abs(first.second - second.second) <= 2,
        )
    }
}
