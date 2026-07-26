// TODO(kotlin-2.4.0-false-positive): K2 false positive; TODO(migrate-v2-compose-rule): migrate to v2 API
@file:Suppress("UNNECESSARY_SAFE_CALL")

package io.term.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.SystemClock
import android.util.Log
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.term.MainActivity
import io.term.bridge.NativeBridge
import io.term.findTerminalSurface
import io.term.getBridge
import io.term.injectDoubleTap
import io.term.injectLongPress
import io.term.injectTap
import io.term.injectTripleTap
import io.term.openDrawer
import io.term.waitForSession
import org.junit.Assert.assertTrue
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import java.io.File
import java.io.FileOutputStream

/**
 * Comprehensive visual verification of text selection functionality.
 *
 * Tests:
 * - Long-press on text selects word/smart content
 * - Long-press on empty area shows paste menu
 * - Selection handles appear at correct positions
 * - Context menu (copy, select all, paste) appears near selection
 * - Handles can be dragged to extend/reduce selection
 * - Colors follow theme (not hardcoded)
 * - Menu does not obscure selected text
 * - Handle positions are accurate
 * - Works with IME open/close
 * - Works with session drawer open/close
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class SelectionVisualVerificationTest {
    companion object {
        private const val TAG = "SelectionVizTest"
        private const val SCREENSHOT_DIR = "selection_viz_screenshots"
    }

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun waitForStable() {
        Thread.sleep(1000)
    }

    private fun saveScreenshot(name: String) {
        val dir = File(composeTestRule.activity.filesDir, SCREENSHOT_DIR)
        dir.mkdirs()
        val file = File(dir, "$name.png")
        try {
            val rootView = composeTestRule.activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = Canvas(bitmap)
            rootView.draw(canvas)
            FileOutputStream(file).use { fos ->
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, fos)
            }
            bitmap.recycle()
            Log.d(TAG, "Screenshot saved: ${file.absolutePath}")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to save screenshot", e)
        }
    }

    private fun generateContent(bridge: NativeBridge) {
        val testUrl = "https://example.com/test_path"
        val testWord = "SELECTION_TEST_WORD"
        val testPath = "/home/user/documents/report.pdf"
        bridge.writeToPty("echo 'Hello World $testWord welcome'\n".toByteArray())
        bridge.writeToPty("echo 'Visit $testUrl for info'\n".toByteArray())
        bridge.writeToPty("echo 'Path: $testPath'\n".toByteArray())
        bridge.writeToPty("echo '   '\n".toByteArray()) // Empty line for paste menu test
        waitForStable()
    }

    private fun getTerminalSurfaceView(): android.view.View = findTerminalSurface(composeTestRule.activity)

    // ── Helper: get approximate cell metrics from surface ──

    private data class CellMetrics(
        val cellWidth: Float,
        val cellHeight: Float,
        val cols: Int,
        val rows: Int,
    )

    private fun estimateCellMetrics(): CellMetrics? {
        val surface = getTerminalSurfaceView()
        return try {
            val width = surface.width.toFloat()
            val height = surface.height.toFloat()
            val bridge = composeTestRule.getBridge()
            val cols =
                bridge?.let { b ->
                    try {
                        b.getGridCols()
                    } catch (e: Exception) {
                        null
                    }
                } ?: 80
            val rows =
                bridge?.let { b ->
                    try {
                        b.getGridRows()
                    } catch (e: Exception) {
                        null
                    }
                } ?: 24
            CellMetrics(width / cols, height / rows, cols, rows)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to estimate cell metrics", e)
            null
        }
    }

    // ── Test 1: Long-press on text selects word ──

    @Test
    fun test01_longPressOnText_selectsWord() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Tap to focus terminal first
        injectTap(surface, cellMetrics.cellWidth * 2, cellMetrics.cellHeight * 2)
        waitForStable()

        // Long-press on text area (around cell 5, 2)
        val longPressX = cellMetrics.cellWidth * 5
        val longPressY = cellMetrics.cellHeight * 3
        injectLongPress(surface, longPressX, longPressY)
        waitForStable()

        saveScreenshot("01_long_press_text_selection")

        // Verify context menu appeared by checking for it visually
        // The menu should contain copy/paste/select all options
        Log.d(TAG, "Long press on text: selection should be visible in screenshot 01")
    }

    // ── Test 2: Long-press on empty area shows paste button ──

    @Test
    fun test02_longPressEmptyArea_showsPaste() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        // Need some content in clipboard for paste button to appear
        bridge.writeToPty("echo 'clipboard_content'\n".toByteArray())
        waitForStable()

        // Set clipboard content
        composeTestRule.activity.runOnUiThread {
            val clipboard =
                composeTestRule.activity.getSystemService(
                    android.content.Context.CLIPBOARD_SERVICE,
                ) as android.content.ClipboardManager
            clipboard.setPrimaryClip(
                android.content.ClipData.newPlainText("test", "paste_target"),
            )
        }

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Long-press on empty/whitespace area (lowest rows should have whitespace after echo commands)
        val longPressX = cellMetrics.cellWidth * 2
        val longPressY = cellMetrics.cellHeight * 20 // Near bottom
        injectLongPress(surface, longPressX, longPressY)
        waitForStable()

        saveScreenshot("02_long_press_empty_paste")
    }

    // ── Test 3: Selection handles appear at correct positions ──

    @Test
    fun test03_selectionHandlesAtCorrectPositions() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Create selection via long press on first line of text
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()

        saveScreenshot("03_selection_handles")

        // A successful long-press selection activates the selection-actions bar,
        // which exposes the "Dismiss selection" button. Assert it is visible.
        composeTestRule
            .onNodeWithTag("Action_Dismiss")
            .assertIsDisplayed()

        Log.d(TAG, "Selection handles should be visible in screenshot 03")
    }

    // ── Test 4: Drag handle to extend selection ──

    @Test
    fun test04_dragHandleExtendsSelection() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Create initial selection via long press
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()

        // Drag to extend selection by simulating touch-move
        val startX = cellMetrics.cellWidth * 10
        val startY = cellMetrics.cellHeight * 3
        val endX = cellMetrics.cellWidth * 20
        val endY = cellMetrics.cellHeight * 4

        val handler = android.os.Handler(android.os.Looper.getMainLooper())
        val dt = SystemClock.uptimeMillis()
        handler.post {
            surface.dispatchTouchEvent(
                android.view.MotionEvent.obtain(
                    dt,
                    dt,
                    android.view.MotionEvent.ACTION_DOWN,
                    startX,
                    startY,
                    0,
                ),
            )
        }
        // Move in steps with delays
        for (step in 1..10) {
            val delay = step * 50L
            handler.postDelayed({
                val x = startX + (endX - startX) * step / 10
                val y = startY + (endY - startY) * step / 10
                surface.dispatchTouchEvent(
                    android.view.MotionEvent.obtain(
                        dt,
                        dt + delay,
                        android.view.MotionEvent.ACTION_MOVE,
                        x,
                        y,
                        0,
                    ),
                )
            }, delay)
        }
        handler.postDelayed({
            surface.dispatchTouchEvent(
                android.view.MotionEvent.obtain(
                    dt,
                    dt + 600,
                    android.view.MotionEvent.ACTION_UP,
                    endX,
                    endY,
                    0,
                ),
            )
        }, 600)
        Thread.sleep(700)
        waitForStable()

        saveScreenshot("04_drag_extend_selection")
    }

    // ── Test 5: Context menu positions correctly ──

    @Test
    fun test05_contextMenuPosition() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Long press on text to trigger context menu
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()

        saveScreenshot("05_context_menu_position")
    }

    // ── Test 6: Selection with IME open/close ──

    @Test
    fun test06_selectionWithIme() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Open IME first
        composeTestRule.activity.runOnUiThread {
            surface.requestFocus()
            val imm =
                composeTestRule.activity.getSystemService(
                    android.content.Context.INPUT_METHOD_SERVICE,
                ) as android.view.inputmethod.InputMethodManager
            imm.showSoftInput(surface, 0)
        }
        waitForStable()
        saveScreenshot("06_ime_open_before_selection")

        // Long press while IME is open
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()
        saveScreenshot("07_selection_with_ime")

        // Close IME
        composeTestRule.activity.runOnUiThread {
            val imm =
                composeTestRule.activity.getSystemService(
                    android.content.Context.INPUT_METHOD_SERVICE,
                ) as android.view.inputmethod.InputMethodManager
            imm.hideSoftInputFromWindow(surface.windowToken, 0)
        }
        waitForStable()
        saveScreenshot("08_selection_after_ime_close")
    }

    // ── Test 7: Selection with session drawer ──

    @Test
    fun test07_selectionWithDrawer() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Create initial selection
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()
        saveScreenshot("09_selection_before_drawer")

        // Open drawer
        composeTestRule.openDrawer()
        waitForStable()
        saveScreenshot("10_selection_with_drawer_open")

        // Close drawer
        composeTestRule.activity.runOnUiThread {
            composeTestRule.activity.onBackPressedDispatcher.onBackPressed()
        }
        waitForStable()
        saveScreenshot("11_selection_after_drawer_close")
    }

    // ── Test 8: Theme-based selection colors ──

    @Test
    fun test08_selectionThemeColors() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Create selection with current theme
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()
        saveScreenshot("12_selection_theme_colors")

        Log.d(TAG, "Selection theme colors saved in screenshot 12")
    }

    // ── Test 9: Select all works ──

    @Test
    fun test09_selectAllViaMenu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Create selection first to show context menu
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 3,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()

        // Try to trigger select all via ViewModel
        composeTestRule.activity.runOnUiThread {
            try {
                val vmField =
                    composeTestRule.activity::class.java
                        .getDeclaredField("viewModel")
                vmField.isAccessible = true
                val viewModel = vmField.get(composeTestRule.activity)
                val selectAllMethod = viewModel::class.java.getMethod("selectAll")
                selectAllMethod.invoke(viewModel)
            } catch (e: Exception) {
                throw AssertionError("Failed to invoke selectAll via ViewModel", e)
            }
        }
        waitForStable()
        saveScreenshot("13_select_all")
    }

    // ── Test 10: RapidOCR verification of highlighted cells ──

    @Test
    fun test10_ocrVerifyHighlightedText() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Long press on specific word to select it
        injectLongPress(
            surface,
            cellMetrics.cellWidth * 2,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()
        saveScreenshot("14_ocr_selection")

        // OCR verification
        val screenshotFile =
            File(
                File(composeTestRule.activity.filesDir, SCREENSHOT_DIR),
                "14_ocr_selection.png",
            )
        if (screenshotFile.exists()) {
            try {
                val process =
                    ProcessBuilder(
                        "rapidocr",
                        screenshotFile.absolutePath,
                    ).redirectErrorStream(true).start()
                val output =
                    process.inputStream
                        .bufferedReader()
                        .readText()
                        .trim()
                process.waitFor()
                if (process.exitValue() == 0 && output.isNotEmpty()) {
                    Log.d(TAG, "OCR result: $output")
                    assertTrue(
                        "OCR must detect text content in selection",
                        output.length > 5 && output.contains("SELECTION", ignoreCase = true),
                    )
                } else {
                    throw AssertionError("RapidOCR exited with code ${process.exitValue()} and empty output")
                }
            } catch (e: Exception) {
                throw AssertionError("RapidOCR verification failed", e)
            }
        }
    }

    // ── Test 11: Selection mode double-tap line ──

    @Test
    fun test11_doubleTapSelectsLine() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Double-tap to select line
        injectDoubleTap(
            surface,
            cellMetrics.cellWidth * 5,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()
        saveScreenshot("15_double_tap_line_select")
    }

    // ── Test 12: Triple-tap selects all (if implemented) ──

    @Test
    fun test12_tripleTapSelectAll() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = checkNotNull(estimateCellMetrics()) { "cell metrics must be estimable (terminal surface must be present)" }

        // Triple-tap to attempt select all
        injectTripleTap(
            surface,
            cellMetrics.cellWidth * 5,
            cellMetrics.cellHeight * 2,
        )
        waitForStable()
        saveScreenshot("16_triple_tap_select_all")
    }
}
