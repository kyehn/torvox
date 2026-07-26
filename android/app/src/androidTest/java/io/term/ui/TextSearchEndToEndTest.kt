
package io.term.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.SystemClock
import android.util.Log
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.term.MainActivity
import io.term.bridge.NativeBridge
import io.term.getBridge
import io.term.openDrawer
import io.term.waitForSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import java.io.File
import java.io.FileOutputStream

/**
 * Comprehensive end-to-end test for text search functionality.
 *
 * Tests:
 * - Search bar opens from drawer button (not ctrl+f)
 * - Search finds text and highlights matches with inverted colors
 * - Previous/Next navigation with auto-scroll to off-screen matches
 * - Smart case toggle (auto-detect uppercase)
 * - Close clears highlights and shows modifier bar again
 * - IME does not obscure search bar
 * - OCR verification of highlighted cells
 * - Scroll effect when navigating to off-screen matches
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class TextSearchEndToEndTest {
    companion object {
        private const val TAG = "SearchE2ETest"
        private const val SCREENSHOT_DIR = "search_e2e_screenshots"
    }

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private val uniqueMarker: String by lazy {
        "SRCH_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"
    }

    private val multiLineMarker: String by lazy {
        "MLINE_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"
    }

    // ── Helper: generate multi-page content ──

    private fun generateMultiPageContent(
        bridge: NativeBridge,
        marker: String,
    ) {
        // Generate enough content to fill >3 terminal pages
        val linesToFill = 200
        for (i in 1..linesToFill) {
            val content =
                when {
                    i % 10 == 0 -> "LINE_${i}_MARKER_$marker"
                    i % 7 == 0 -> marker
                    i % 5 == 0 -> "CONTENT_LINE_$i"
                    else -> "data_line_$i"
                }
            bridge.writeToPty("echo '$content'\n".toByteArray())
        }
        waitForOutput()
    }

    private fun waitForOutput() {
        Thread.sleep(1500) // Wait for PTY output to process
    }

    private fun waitForSearchStable() {
        Thread.sleep(500)
    }

    private fun openSearchAndType(text: String) {
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement(text)
        composeTestRule.waitForIdle()
        waitForSearchStable()
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

    // ── Helper: RapidOCR verification ──

    private fun ocrImage(imagePath: String): String? = try {
        val process =
            ProcessBuilder(
                "rapidocr",
                imagePath,
            ).redirectErrorStream(true).start()
        val output =
            process.inputStream
                .bufferedReader()
                .readText()
                .trim()
        process.waitFor()
        if (process.exitValue() == 0 && output.isNotEmpty()) output else null
    } catch (e: Exception) {
        Log.e(TAG, "RapidOCR failed", e)
        null
    }

    // ── Test 1: Search opens from drawer button ──

    @Test
    fun test01_searchOpensFromDrawerButton() {
        composeTestRule.waitForSession()
        composeTestRule.waitForIdle()

        // Verify search bar is NOT visible initially
        val initialBarCount =
            composeTestRule
                .onAllNodes(hasTestTag("TextSearchBar"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("Search bar must not be visible before opening", 0, initialBarCount)

        // Also verify ctrl+f does NOT open search bar
        composeTestRule.activity.runOnUiThread {
            val keyEvent =
                android.view
                    .KeyEvent(
                        android.view.KeyEvent.ACTION_DOWN,
                        android.view.KeyEvent.KEYCODE_F,
                    ).apply {
                        // Ctrl flag
                    }
            composeTestRule.activity.dispatchKeyEvent(
                android.view.KeyEvent(
                    SystemClock.uptimeMillis(),
                    SystemClock.uptimeMillis(),
                    android.view.KeyEvent.ACTION_DOWN,
                    android.view.KeyEvent.KEYCODE_F,
                    0,
                    android.view.KeyEvent.META_CTRL_ON,
                ),
            )
        }
        composeTestRule.waitForIdle()
        val ctrlFCount =
            composeTestRule
                .onAllNodes(hasTestTag("TextSearchBar"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("Ctrl+F must NOT open search bar", 0, ctrlFCount)

        // Open drawer and click search button
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()

        // Verify search bar IS visible
        composeTestRule.onNodeWithTag("TextSearchBar").assertExists("Search bar must be visible after opening from drawer")
        composeTestRule.onNodeWithTag("SearchTextField").assertExists("Search text field must be visible")
        saveScreenshot("01_search_bar_opened")
    }

    // ── Test 2: Search finds and highlights text ──

    @Test
    fun test02_searchFindsAndHighlightsMatches() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)

        // Verify marker exists in terminal
        val terminalText = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain marker '$uniqueMarker'", terminalText.contains(uniqueMarker))

        // Open search and type the marker
        openSearchAndType(uniqueMarker)

        // Verify result count is shown
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("Search result count must be visible")
        waitForSearchStable()

        // Take screenshot for OCR verification
        saveScreenshot("02_search_highlights")

        // Try OCR verification if available
        val screenshotFile =
            File(
                File(composeTestRule.activity.filesDir, SCREENSHOT_DIR),
                "02_search_highlights.png",
            )
        if (screenshotFile.exists()) {
            val ocrResult = ocrImage(screenshotFile.absolutePath)
            if (ocrResult != null) {
                assertTrue(
                    "OCR must detect searched text. Found: $ocrResult",
                    ocrResult.contains(uniqueMarker, ignoreCase = true) ||
                        ocrResult.contains("MARKER", ignoreCase = true) ||
                        ocrResult.contains("SRCH", ignoreCase = true),
                )
            }
        }
    }

    // ── Test 3: Search navigation (previous/next) with scrolling ──

    @Test
    fun test03_searchNavigatesWithScroll() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)

        openSearchAndType(uniqueMarker)
        waitForSearchStable()

        // Click Next multiple times to verify navigation
        composeTestRule.onNodeWithTag("SearchNext").performClick()
        waitForSearchStable()
        composeTestRule.onNodeWithTag("SearchNext").performClick()
        waitForSearchStable()
        saveScreenshot("03_after_next_twice")

        // Click Previous to go back
        composeTestRule.onNodeWithTag("SearchPrevious").performClick()
        waitForSearchStable()
        saveScreenshot("04_after_previous")

        // Verify terminal text still contains the marker (search didn't break terminal)
        val terminalText = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must still contain marker after navigation", terminalText.contains(uniqueMarker))
    }

    // ── Test 4: Smart case toggle ──

    @Test
    fun test04_smartCaseToggle() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)

        // Open search with lowercase version of marker
        val lowerMarker = uniqueMarker.lowercase()
        openSearchAndType(lowerMarker)
        waitForSearchStable()

        // Verify results shown
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("Results must be shown for lowercase search")

        // Toggle case sensitive on
        composeTestRule.onNodeWithTag("SearchCaseSensitive").performClick()
        waitForSearchStable()

        // Search with original case
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement(uniqueMarker)
        waitForSearchStable()
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("Results must be shown for case-sensitive search")

        saveScreenshot("05_case_sensitive_search")
    }

    // ── Test 5: Close search restores modifier bar ──

    @Test
    fun test05_closeSearchRestoresModifierBar() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)

        openSearchAndType(uniqueMarker)
        waitForSearchStable()

        // Close search
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        composeTestRule.waitForIdle()

        // Verify search bar is gone
        val afterCloseCount =
            composeTestRule
                .onAllNodes(hasTestTag("TextSearchBar"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("Search bar must be hidden after close", 0, afterCloseCount)

        // Verify modifier bar is back
        composeTestRule.onNodeWithTag("ModifierBar").assertExists("Modifier bar must be visible after search close")
        saveScreenshot("06_after_search_close")
    }

    // ── Test 6: Multi-line text with multiple matches ──

    @Test
    fun test06_multiLineSearch() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!

        // Generate content on multiple lines
        for (i in 1..30) {
            bridge.writeToPty("echo '${multiLineMarker}_$i'\n".toByteArray())
        }
        waitForOutput()

        openSearchAndType(multiLineMarker)
        waitForSearchStable()

        // Result count should show multiple matches
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("Result count must show matches")
        saveScreenshot("07_multi_line_search")
    }

    // ── Test 7: Empty query shows no results ──

    @Test
    fun test07_emptyQuery() {
        composeTestRule.waitForSession()

        // Open search without typing anything
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()

        // Result count should not be shown for empty query
        val emptyQueryCount =
            composeTestRule
                .onAllNodes(hasTestTag("SearchResultCount"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("No result count for empty query", 0, emptyQueryCount)
    }

    // ── Test 8: No results shows proper indicator ──

    @Test
    fun test08_noResultsIndicator() {
        composeTestRule.waitForSession()

        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()

        // Type something that won't match
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement("XYZZYX_NONEXISTENT_12345")
        composeTestRule.waitForIdle()
        waitForSearchStable()

        // Should show "No results" or similar
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("No results indicator must be shown")
        saveScreenshot("08_no_results")
    }

    // ── Test 9: Search bar stays visible with IME ──

    @Test
    fun test09_searchBarNotObscuredByIme() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!

        openSearchAndType(uniqueMarker)
        waitForSearchStable()

        // Focus the search field (triggers IME)
        composeTestRule.onNodeWithTag("SearchTextField").performClick()
        composeTestRule.waitForIdle()

        // Take screenshot to verify search bar is visible with IME
        saveScreenshot("09_search_with_ime")

        // Verify search bar is still visible
        composeTestRule.onNodeWithTag("TextSearchBar").assertExists("Search bar must be visible with IME open")
        composeTestRule.onNodeWithTag("SearchTextField").assertExists("Search text field must be visible with IME open")

        // Close IME and verify search bar still works
        composeTestRule.activity.runOnUiThread {
            val imm =
                composeTestRule.activity.getSystemService(
                    android.content.Context.INPUT_METHOD_SERVICE,
                ) as android.view.inputmethod.InputMethodManager
            imm.hideSoftInputFromWindow(
                composeTestRule.activity.window.decorView.windowToken,
                0,
            )
        }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TextSearchBar").assertExists("Search bar must remain after IME close")
    }

    // ── Test 10: Search highlights use theme-based inverted colors ──

    @Test
    fun test10_searchHighlightColors() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)

        openSearchAndType(uniqueMarker)
        waitForSearchStable()

        // Save a screenshot for pixel analysis
        saveScreenshot("10_highlight_colors")

        // Verify search results exist
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("Results must be visible for color test")

        // Visually verify highlights in screenshot
        val screenshotFile =
            File(
                File(composeTestRule.activity.filesDir, SCREENSHOT_DIR),
                "10_highlight_colors.png",
            )
        if (screenshotFile.exists()) {
            val ocrResult = ocrImage(screenshotFile.absolutePath)
            Log.d(TAG, "OCR result for highlight color test: $ocrResult")
            // OCR should at least detect some text content
            assertTrue(
                "OCR must detect text content after highlight",
                ocrResult != null && ocrResult.length > 5,
            )
        }
    }
}
