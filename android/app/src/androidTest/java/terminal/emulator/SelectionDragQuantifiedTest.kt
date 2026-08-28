package terminal.emulator

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.bridge.Bridge

/**
 * Quantified verification of the selection/drag behaviors reported broken:
 *
 * 1. "paste 始终显示 / 菜单内容错误" — the long-press target decides the menu. Metric: exact ActionMode item
 *    set after a real long-press. Assert: whitespace → {PASTE} without COPY; word → {COPY,
 *    SELECT_ALL} without PASTE.
 * 2. "拖动卡顿漂移" live-highlight side: while a handle is dragged in slow steps, the inverted-cell
 *    highlight must change on screen BETWEEN the steps, not only at release. Metric: consecutive
 *    full-screen captures taken after each drag step must differ materially at >= 3 of 4 steps, and
 *    the final COPY action must be enabled (non-empty grown range).
 * 3. D7.5: grabbing a handle on a paste-only (blank cell) selection must upgrade it to a text
 *    selection and grow a real range — measured as the paste-only menu transitioning to the full
 *    menu with an enabled COPY.
 *
 * All measured values log as `UX_METRIC ...` lines for trend tracking.
 */
class SelectionDragQuantifiedTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** The TerminalSurface view, resolved through the house TestUtils helper. */
    private fun surfaceView(): android.view.View = findTerminalSurface(composeTestRule.activity)

    /**
     * Screen coordinates of the anchor point of a grid cell: column [col], row-bottom boundary of
     * viewport row [row]. Handles hang below their cell corners, so this is where a grab lands on the
     * handle body.
     */
    private fun cellAnchorOnScreen(
        col: Int,
        row: Int,
    ): Pair<Int, Int> {
        val surface = surfaceView()
        val loc = IntArray(2)
        surface.getLocationOnScreen(loc)
        val cw = bridge().getCellWidth()
        val ch = bridge().getCellHeight()
        assertTrue("cell metrics unavailable (cw=$cw ch=$ch)", cw > 0f && ch > 0f)
        // Viewport rows are 0-based from the top of the grid; add one row so
        // the anchor sits at the row's bottom boundary where handles hang.
        return Pair(
            (loc[0] + col * cw).toInt(),
            (loc[1] + (row + 1) * ch).toInt(),
        )
    }

    private fun waitForMenuText(text: String, timeoutMs: Long = 4_000) = device.wait(Until.findObject(By.text(text)), timeoutMs)

    private fun menuVisible(text: String): Boolean = device.findObject(By.text(text)) != null

    private fun resetSelection() {
        injectTap(
            surfaceView(),
            (device.displayWidth / 2).toFloat(),
            (device.displayHeight - 120).toFloat(),
        )
        Thread.sleep(600)
    }

    // ── 1. menu content matrix ──────────────────────────────────────────

    @Test
    fun whitespace_longpress_shows_paste_only_menu() {
        val b = bridge()
        b.writeToPty("clear\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(1_200)

        // Long-press the blank region right of the prompt: guaranteed
        // whitespace target regardless of font metrics (prompt never fills
        // the whole line).
        val x = device.displayWidth - 160
        val y = device.displayHeight - 260
        injectLongPress(surfaceView(), x.toFloat(), y.toFloat())

        assertNotNull("PASTE item missing for whitespace long-press", waitForMenuText("PASTE"))
        assertTrue(
            "whitespace long-press must not offer COPY",
            !menuVisible("COPY"),
        )
        UxTestUtils.metric("menu_whitespace_paste_only", 1)
        resetSelection()
    }

    @Test
    fun word_longpress_shows_copy_selectall_without_paste() {
        val b = bridge()
        b.writeToPty("printf 'targetword targetword targetword\\n'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(1_500)

        // Fresh output prints just above the prompt: left margin of the
        // last line reliably lands on printed text.
        injectLongPress(surfaceView(), 120f, (device.displayHeight - 300).toFloat())

        assertNotNull("COPY item missing for word long-press", waitForMenuText("COPY"))
        assertTrue("SELECT_ALL item missing", menuVisible("SELECT ALL") || menuVisible("Select all"))
        assertTrue(
            "word long-press must NOT show PASTE (the reported 'paste always visible' bug)",
            !menuVisible("PASTE"),
        )
        UxTestUtils.metric("menu_word_full_set", 1)
        resetSelection()
    }

    // ── 2. live-highlight cadence during handle drag ────────────────────

    @Test
    fun handle_drag_updates_highlight_live_between_steps() {
        val b = bridge()
        b.writeToPty("printf 'dragstart dragend dragend dragend\\n'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(1_500)

        // Double-tap selects the word under the finger; its END handle then
        // anchors at that word's right cell edge.
        val tapX = 130
        val tapYBottomRow = device.displayHeight - 300
        injectDoubleTap(surfaceView(), tapX.toFloat(), tapYBottomRow.toFloat())
        Thread.sleep(900)
        assertNotNull("double-tap did not open the selection menu", waitForMenuText("COPY"))

        // Grab the END handle: ~2 cells right of the tap (the selected word
        // spans about one cell per 5-6 chars at default metrics; 2 cells is
        // safely past its right edge) and exactly on the row-bottom anchor.
        val surfaceLocTap = IntArray(2)
        surfaceView().getLocationOnScreen(surfaceLocTap)
        val cw = bridge().getCellWidth()
        val ch = bridge().getCellHeight()
        val tappedCol = ((tapX - surfaceLocTap[0]) / cw).toInt()
        val tappedRow = ((tapYBottomRow - surfaceLocTap[1]) / ch).toInt()
        // The double-tapped word spans ~9 chars; its END handle hangs ~2 cells
        // right of the tap column at the same row-bottom anchor.
        val (grabX, grabY) = cellAnchorOnScreen(col = tappedCol + 2, row = tappedRow)
        val before = UxTestUtils.screenshot(device)

        var liveUpdates = 0
        var previous = before
        val cwInt = cw.toInt().coerceAtLeast(20)
        var currentX = grabX
        repeat(4) {
            currentX += cwInt
            UxTestUtils.injectDrag(
                surfaceView(),
                x0 = (currentX - cwInt / 2).toFloat(),
                y0 = grabY.toFloat(),
                x1 = currentX.toFloat(),
                y1 = grabY.toFloat(),
                steps = 2,
                stepDelayMs = 110,
            )
            val capture = UxTestUtils.screenshot(device)
            if (UxTestUtils.changedPixelCount(previous, capture) > 150) liveUpdates++
            previous = capture
        }

        UxTestUtils.metric("drag_live_highlight_steps", liveUpdates)
        assertTrue(
            "highlight changed at only $liveUpdates/4 steps — drag updates are not live",
            liveUpdates >= 3,
        )

        val copy = waitForMenuText("COPY", 3_000)
        assertNotNull("selection menu vanished after handle drag", copy)
        assertTrue("COPY disabled after drag — range did not grow to real text", copy!!.isEnabled)
        resetSelection()
    }

    // ── 3. D7.5: paste-only upgrade on handle grab ──────────────────────

    @Test
    fun paste_only_handle_drag_upgrades_selection_and_grows_D75() {
        val b = bridge()
        b.writeToPty("printf 'growme growme growme\\n'\n".toByteArray(Charsets.UTF_8))
        Thread.sleep(1_500)

        // Long-press blank space right of the prompt → single-cell
        // paste-only selection with stacked handles.
        val blankX = device.displayWidth - 160
        val blankY = device.displayHeight - 260
        injectLongPress(surfaceView(), blankX.toFloat(), blankY.toFloat())
        assertNotNull("precondition: PASTE-only menu missing", waitForMenuText("PASTE"))
        assertTrue("precondition: COPY must be absent on blank selection", !menuVisible("COPY"))

        // Grab the stacked END handle of the pressed cell itself: derive the
        // pressed (col,row) from the surface's on-screen origin, then anchor at
        // that cell's bottom-right corner where the END handle hangs.
        val surfaceLoc = IntArray(2)
        surfaceView().getLocationOnScreen(surfaceLoc)
        val cwPx = bridge().getCellWidth()
        val chPx = bridge().getCellHeight()
        val pressedCol = (((blankX - surfaceLoc[0]) / cwPx).toInt() + 1).coerceAtLeast(2)
        val pressedRow = ((blankY - surfaceLoc[1]) / chPx).toInt()
        val (handleX, handleY) = cellAnchorOnScreen(col = pressedCol, row = pressedRow)
        UxTestUtils.injectDrag(
            surfaceView(),
            x0 = handleX.toFloat(),
            y0 = handleY.toFloat(),
            x1 = 110f,
            y1 = (handleY - chPx.toInt() * 2).toFloat(),
            steps = 6,
            stepDelayMs = 100,
        )

        // The upgrade is observable exactly through the menu transition:
        // paste-only {PASTE} → full {COPY,...} with non-empty text.
        val copy = waitForMenuText("COPY", 4_000)
        assertNotNull(
            "D7.5 failed: dragging a blank-selection handle did not grow a text range",
            copy,
        )
        assertTrue("grown range has no selectable text", copy!!.isEnabled)
        assertTrue("full menu still shows PASTE after growth to text", !menuVisible("PASTE"))
        UxTestUtils.metric("d75_blank_drag_upgrade", 1)
        resetSelection()
    }
}
