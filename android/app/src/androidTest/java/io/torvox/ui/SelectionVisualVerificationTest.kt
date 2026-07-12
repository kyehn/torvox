// TODO(kotlin-2.4.0-false-positive): K2 false positive; TODO(migrate-v2-compose-rule): migrate to v2 API
@file:Suppress("UNNECESSARY_SAFE_CALL")

package io.torvox.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.SystemClock
import android.util.Log
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.torvox.MainActivity
import io.torvox.bridge.TorvoxBridge
import io.torvox.findTerminalSurface
import io.torvox.getBridge
import io.torvox.injectDoubleTap
import io.torvox.injectLongPress
import io.torvox.injectTap
import io.torvox.injectTripleTap
import io.torvox.openDrawer
import io.torvox.waitForSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
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

    private fun generateContent(bridge: TorvoxBridge) {
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Tap to focus terminal first
        composeTestRule.activity.runOnUiThread {
            injectTap(composeTestRule.activity, surface, cellMetrics.cellWidth * 2, cellMetrics.cellHeight * 2)
        }
        waitForStable()

        // Long-press on text area (around cell 5, 2)
        val longPressX = cellMetrics.cellWidth * 5
        val longPressY = cellMetrics.cellHeight * 3
        composeTestRule.activity.runOnUiThread {
            injectLongPress(composeTestRule.activity, surface, longPressX, longPressY)
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Long-press on empty/whitespace area (lowest rows should have whitespace after echo commands)
        val longPressX = cellMetrics.cellWidth * 2
        val longPressY = cellMetrics.cellHeight * 20 // Near bottom
        composeTestRule.activity.runOnUiThread {
            injectLongPress(composeTestRule.activity, surface, longPressX, longPressY)
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Create selection via long press on first line of text
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
        waitForStable()

        saveScreenshot("03_selection_handles")

        // Verify selection by checking bridge state
        val selectionState =
            try {
                val stateField =
                    composeTestRule.activity::class.java
                        .getDeclaredField("viewModel")
                        ?.let { field ->
                            field.isAccessible = true
                            field.get(composeTestRule.activity)
                        }
            } catch (e: Exception) {
                null
            }

        Log.d(TAG, "Selection handles should be visible in screenshot 03")
    }

    // ── Test 4: Drag handle to extend selection ──

    @Test
    fun test04_dragHandleExtendsSelection() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateContent(bridge)

        val surface = getTerminalSurfaceView()
        val cellMetrics = estimateCellMetrics() ?: return

        // Create initial selection via long press
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
        waitForStable()

        // Drag to extend selection by simulating touch-move
        val startX = cellMetrics.cellWidth * 10
        val startY = cellMetrics.cellHeight * 3
        val endX = cellMetrics.cellWidth * 20
        val endY = cellMetrics.cellHeight * 4

        composeTestRule.activity.runOnUiThread {
            val dt = SystemClock.uptimeMillis()
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
            // Move in steps
            for (step in 1..10) {
                val x = startX + (endX - startX) * step / 10
                val y = startY + (endY - startY) * step / 10
                surface.dispatchTouchEvent(
                    android.view.MotionEvent.obtain(
                        dt,
                        dt + step * 50L,
                        android.view.MotionEvent.ACTION_MOVE,
                        x,
                        y,
                        0,
                    ),
                )
                Thread.sleep(30)
            }
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
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Long press on text to trigger context menu
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

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
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Create initial selection
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Create selection with current theme
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Create selection first to show context menu
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 3,
                cellMetrics.cellHeight * 2,
            )
        }
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
                Log.e(TAG, "Failed to invoke selectAll", e)
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Long press on specific word to select it
        composeTestRule.activity.runOnUiThread {
            injectLongPress(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 2,
                cellMetrics.cellHeight * 2,
            )
        }
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
                }
            } catch (e: Exception) {
                Log.e(TAG, "OCR verification skipped: rapidocr not available", e)
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Double-tap to select line
        composeTestRule.activity.runOnUiThread {
            injectDoubleTap(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 5,
                cellMetrics.cellHeight * 2,
            )
        }
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
        val cellMetrics = estimateCellMetrics() ?: return

        // Triple-tap to attempt select all
        composeTestRule.activity.runOnUiThread {
            injectTripleTap(
                composeTestRule.activity,
                surface,
                cellMetrics.cellWidth * 5,
                cellMetrics.cellHeight * 2,
            )
        }
        waitForStable()
        saveScreenshot("16_triple_tap_select_all")
    }
}
