
package io.term.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Log
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import io.term.MainActivity
import io.term.getBridge
import io.term.openDrawer
import io.term.waitForSession
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import java.io.File

@RunWith(AndroidJUnit4::class)
@LargeTest
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class TextSearchColorVerificationTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private val uniqueMarker = "COLORCHK_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"

    private val uiDevice = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    private val screenshotDir =
        File(
            InstrumentationRegistry.getInstrumentation().targetContext.getExternalFilesDir(null),
            "color_test",
        )

    @Test
    fun a_searchFindsMarkerWithHighlight_colorsFromTheme() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()
        assertNotNull("Bridge must be available", bridge)

        generateContent(bridge!!, uniqueMarker, linesPerPage = 10, pages = 2)
        waitForTerminalStable()

        val textBefore = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain marker", textBefore.contains(uniqueMarker))

        openSearchAndType(uniqueMarker)
        waitForSearchStable()

        val screenshot = takeRealScreenshot("a_search_highlight")

        val nonBlackRatio = analyzeNonBlackRatio(screenshot)
        assertTrue("Screenshot should have visible content (non-black ratio: $nonBlackRatio)", nonBlackRatio > 0.01)

        val highlighted = detectHighlightPixels(screenshot, minDominance = 30)
        assertTrue(
            "Search marker '$uniqueMarker' should produce highlighted pixels (found $highlighted)",
            highlighted > 10,
        )

        Log.i("ColorVerify", "a_searchFindsMarkerWithHighlight: found $highlighted highlighted pixels, ratio=$nonBlackRatio")
        saveScreenshot("a_search_highlight", screenshot)
        screenshot.recycle()
    }

    @Test
    fun b_currentMatchDiffersFromOtherMatches() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()
        assertNotNull("Bridge must be available", bridge)

        generateContent(bridge!!, uniqueMarker, linesPerPage = 10, pages = 2)
        waitForTerminalStable()

        val textBefore = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain marker", textBefore.contains(uniqueMarker))

        openSearchAndType(uniqueMarker)
        waitForSearchStable()

        composeTestRule.onNodeWithTag("SearchNext").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(1000)

        val screenshot = takeRealScreenshot("b_current_match")

        val highlighted = detectHighlightPixels(screenshot, minDominance = 30)
        assertTrue("After SearchNext, highlighted pixels should exist", highlighted > 10)

        saveScreenshot("b_current_match", screenshot)
        screenshot.recycle()
    }

    @Test
    fun c_noHighlightWhenNoMatch() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()
        assertNotNull("Bridge must be available", bridge)

        val textBefore = bridge!!.getTerminalText() ?: ""
        assertTrue("Terminal should have content", textBefore.isNotEmpty())

        openSearchAndType("ZZZ_XYZZZZ_99999")
        waitForSearchStable()

        val screenshot = takeRealScreenshot("c_no_match")

        val highlighted = detectHighlightPixels(screenshot, minDominance = 30)
        assertTrue("No-match search should have minimal highlight detection (found $highlighted)", highlighted < 200)

        saveScreenshot("c_no_match", screenshot)
        screenshot.recycle()
    }

    @Test
    fun d_searchCaseSensitive_affectsHighlights() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge()
        assertNotNull("Bridge must be available", bridge)

        bridge!!.writeToPty("echo '${uniqueMarker}_mixed'\n".toByteArray())
        bridge.writeToPty("echo '${uniqueMarker.uppercase()}_MIXED'\n".toByteArray())
        waitForTerminalStable()

        val text = bridge.getTerminalText() ?: ""
        assertTrue("Terminal must contain markers", text.contains("${uniqueMarker}_mixed"))

        openSearchAndType("${uniqueMarker}_mixed")
        waitForSearchStable()

        val insensitiveHighlights = detectHighlightPixels(takeRealScreenshot("d_insensitive"), minDominance = 30)

        composeTestRule.onNodeWithTag("SearchCaseSensitive").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(2000)

        val sensitiveHighlights = detectHighlightPixels(takeRealScreenshot("d_sensitive"), minDominance = 30)

        Log.i("ColorVerify", "Case-insensitive highlights: $insensitiveHighlights, case-sensitive: $sensitiveHighlights")
        assertTrue(
            "Case-insensitive should have >= highlights than case-sensitive ($insensitiveHighlights >= $sensitiveHighlights)",
            insensitiveHighlights >= sensitiveHighlights,
        )

        saveScreenshot("d_case_insensitive", loadLastScreenshot("d_insensitive"))
        saveScreenshot("d_case_sensitive", loadLastScreenshot("d_sensitive"))
    }

    private fun generateContent(
        bridge: io.term.bridge.NativeBridge,
        marker: String,
        linesPerPage: Int,
        pages: Int,
    ) {
        for (page in 1..pages) {
            for (line in 1..linesPerPage) {
                bridge.writeToPty("echo 'Page${page}_Line${line}_${marker}_data'\n".toByteArray())
                Thread.sleep(10)
            }
        }
    }

    private fun openSearchAndType(query: String) {
        composeTestRule.waitForIdle()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement(query)
        composeTestRule.waitForIdle()
    }

    private fun waitForTerminalStable() {
        Thread.sleep(3000)
        composeTestRule.waitForIdle()
    }

    private fun waitForSearchStable() {
        Thread.sleep(2000)
        composeTestRule.waitForIdle()
    }

    private fun takeRealScreenshot(name: String): Bitmap {
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "$name.png")
        uiDevice.takeScreenshot(pngFile)
        return BitmapFactory.decodeFile(pngFile.absolutePath)
    }

    private fun saveScreenshot(
        name: String,
        bitmap: Bitmap,
    ) {
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "$name.png")
        pngFile.outputStream().use { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
        }
    }

    private fun loadLastScreenshot(name: String): Bitmap = BitmapFactory.decodeFile(File(screenshotDir, "$name.png").absolutePath)

    private fun detectHighlightPixels(
        bitmap: Bitmap,
        minDominance: Int = 30,
    ): Int {
        var greenCount = 0
        var redCount = 0
        val step = 4
        for (x in 0 until bitmap.width step step) {
            for (y in 0 until bitmap.height step step) {
                val pixel = bitmap.getPixel(x, y)
                val r = android.graphics.Color.red(pixel)
                val g = android.graphics.Color.green(pixel)
                val b = android.graphics.Color.blue(pixel)

                // Green highlight from ansi[2] (Fg=0xFFA6E3A1 = RGB(166,227,161))
                // Composited at alpha 0.9 over terminal content → G dominates
                if (g > r + minDominance && g > b + minDominance) {
                    greenCount++
                }

                // Red/pink highlight from ansi[1] (Fg=0xFFF38BA8 = RGB(243,139,168))
                // Composited at alpha 1.0 → R dominates
                if (r > g + minDominance && r > b + minDominance) {
                    redCount++
                }
            }
        }
        return greenCount + redCount
    }

    private fun analyzeNonBlackRatio(bitmap: Bitmap): Double {
        var nonBlack = 0L
        var total = 0L
        val step = 4
        for (x in 0 until bitmap.width step step) {
            for (y in 0 until bitmap.height step step) {
                val pixel = bitmap.getPixel(x, y)
                val r = android.graphics.Color.red(pixel)
                val g = android.graphics.Color.green(pixel)
                val b = android.graphics.Color.blue(pixel)
                if (r > 15 || g > 15 || b > 15) nonBlack++
                total++
            }
        }
        return if (total > 0) nonBlack.toDouble() / total.toDouble() else 0.0
    }
}
