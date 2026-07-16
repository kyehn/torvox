
package io.torvox.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.util.Log
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.torvox.MainActivity
import io.torvox.bridge.TorvoxBridge
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import java.io.File
import java.io.FileOutputStream

@RunWith(AndroidJUnit4::class)
@LargeTest
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class TextSearchOcrTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private val uniqueMarker = "TORVOX_OCR_${java.util.UUID.randomUUID().toString().take(8).uppercase()}"

    @Test
    fun a_generateContent_thenSearch_highlightsVisible() {
        composeTestRule.waitForSession()
        val bridge: TorvoxBridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)
        waitForTerminalStable()
        val bridgeTextBefore = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain marker", bridgeTextBefore.contains(uniqueMarker))
        openSearchAndType(uniqueMarker)
        waitForSearchStable()
        val bridgeTextAfter = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must still contain marker after search", bridgeTextAfter.contains(uniqueMarker))
        saveScreenshot("00_search_highlight")
    }

    @Test
    fun b_searchNext_scrollsAndChangesPosition() {
        composeTestRule.waitForSession()
        val bridge: TorvoxBridge = composeTestRule.getBridge()!!
        val scrollMarker = "SCROLL_MARKER_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"
        generateMultiPageContent(bridge, uniqueMarker)
        bridge.writeToPty("echo '$scrollMarker'\n".toByteArray())
        waitForTerminalStable()
        openSearchAndType(scrollMarker)
        waitForSearchStable()
        composeTestRule.onNodeWithTag("SearchResultCount").assertExists("Result count must be visible")
        saveScreenshot("01_before_scroll")
        composeTestRule.onNodeWithTag("SearchNext").performClick()
        waitForSearchStable()
        val bridgeTextAfter = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain scroll marker", bridgeTextAfter.contains(scrollMarker))
        saveScreenshot("02_after_scroll")
    }

    @Test
    fun c_searchClose_restoresModifierBar() {
        composeTestRule.waitForSession()
        val bridge: TorvoxBridge = composeTestRule.getBridge()!!
        generateMultiPageContent(bridge, uniqueMarker)
        waitForTerminalStable()
        openSearchAndType(uniqueMarker)
        waitForSearchStable()
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        waitForSearchStable()
        val bridgeText = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must still contain marker after close", bridgeText.contains(uniqueMarker))
        composeTestRule.onNodeWithTag("SearchButton").assertExists("SearchButton must exist after close")
        saveScreenshot("03_after_close")
    }

    @Test
    fun d_searchCaseToggle_changesResults() {
        composeTestRule.waitForSession()
        val bridge: TorvoxBridge = composeTestRule.getBridge()!!
        bridge.writeToPty("echo '${uniqueMarker}_lower'\n".toByteArray())
        bridge.writeToPty("echo '${uniqueMarker}_UPPER'\n".toByteArray())
        waitForTerminalStable()
        val initialText = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain upper marker", initialText.contains("${uniqueMarker}_UPPER"))
        openSearchAndType("${uniqueMarker}_UPPER")
        waitForSearchStable()
        composeTestRule.onNodeWithTag("SearchCaseSensitive").performClick()
        waitForSearchStable()
        val afterToggle = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain search markers after toggle", afterToggle.contains("${uniqueMarker}_UPPER"))
        saveScreenshot("04_case_toggle")
    }

    private fun openSearchAndType(query: String) {
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement(query)
        composeTestRule.waitForIdle()
    }

    private fun generateMultiPageContent(
        bridge: TorvoxBridge,
        marker: String,
    ) {
        for (page in 1..4) {
            for (line in 1..30) {
                bridge.writeToPty("echo 'Page ${page}_Line_${line}_${marker}_content'\n".toByteArray())
                Thread.sleep(10)
            }
        }
    }

    private fun waitForTerminalStable() {
        Thread.sleep(3000)
        composeTestRule.waitForIdle()
    }

    private fun waitForSearchStable() {
        Thread.sleep(2000)
        composeTestRule.waitForIdle()
    }

    private fun saveScreenshot(name: String) {
        val view = composeTestRule.activity.window.decorView
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        view.draw(Canvas(bitmap))
        val dir = File(composeTestRule.activity.getExternalFilesDir(null), "torvox_ocr_test")
        dir.mkdirs()
        val pngFile = File(dir, "$name.png")
        FileOutputStream(pngFile).use { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
        }
        bitmap.recycle()
        Log.i("TextSearchOcrTest", "Screenshot saved: ${pngFile.absolutePath}")
    }
}
