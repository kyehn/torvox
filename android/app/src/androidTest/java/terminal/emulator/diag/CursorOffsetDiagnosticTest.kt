package terminal.emulator.diag

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.grantNotificationPermission

/**
 * diagnostic: correlates the render-source cursor coordinate with the block actually visible in the
 * screenshot, stage by stage, to attribute the "cursor one row below the text" report to the VT
 * data or the render thread.
 *
 * All evidence goes to logcat tag DiagGrid (scoped storage hides app external dirs from adb on API
 * 35).
 */
class CursorOffsetDiagnosticTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        // Deliberately NOT using waitForSession(): it sets test.minSurface=true
        // which bypasses the small-surface guard and distorts grid geometry.
        grantNotificationPermission()
        composeTestRule.waitUntil(timeoutMillis = 60_000) {
            try {
                composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
                true
            } catch (e: AssertionError) {
                false
            } catch (e: Exception) {
                false
            }
        }
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** Bbox of a lone bright block (~1 cell wide) below the status bar. */
    private fun loneBlockBbox(): String {
        val shot = device.takeScreenshot() ?: return "no-shot"
        val w = shot.width
        val h = shot.height
        val pixels = IntArray(w * h)
        shot.getPixels(pixels, 0, w, 0, 0, w, h)
        var minX = Int.MAX_VALUE
        var maxX = -1
        var minY = Int.MAX_VALUE
        var maxY = -1
        for (y in 100 until 1800) {
            var rowCount = 0
            var rowMinX = Int.MAX_VALUE
            var rowMaxX = -1
            for (x in 0 until w) {
                val p = pixels[y * w + x]
                val lum = ((p shr 16 and 0xFF) * 299 + (p shr 8 and 0xFF) * 587 + (p and 0xFF) * 114) / 1000
                if (lum > 140) {
                    rowCount++
                    if (x < rowMinX) rowMinX = x
                    if (x > rowMaxX) rowMaxX = x
                }
            }
            // A lone cursor block spans roughly one cell width; text rows span
            // the full grid and are excluded by the narrow-count window.
            if (rowCount in 10..60) {
                if (y < minY) minY = y
                if (y > maxY) maxY = y
                if (rowMinX < minX) minX = rowMinX
                if (rowMaxX > maxX) maxX = rowMaxX
            }
        }
        if (maxX < 0) return "none"
        return "bbox=x[$minX,$maxX]y[$minY,$maxY]"
    }

    private fun logRowProfile(stage: String) {
        val shot = device.takeScreenshot() ?: return
        val w = shot.width
        val pixels = IntArray(w * 260)
        shot.getPixels(pixels, 0, w, 0, 0, w, 260)
        val profile = StringBuilder()
        for (y in 100 until 260 step 4) {
            var count = 0
            var minX = -1
            var maxX = -1
            for (x in 0 until w) {
                val p = pixels[y * w + x]
                val lum = ((p shr 16 and 0xFF) * 299 + (p shr 8 and 0xFF) * 587 + (p and 0xFF) * 114) / 1000
                if (lum > 140) {
                    count++
                    if (minX < 0) minX = x
                    maxX = x
                }
            }
            if (count > 0) profile.append("y$y:$count[$minX-$maxX] ")
        }
        android.util.Log.i("DiagGrid", "$stage PROFILE: $profile")
    }

    private fun logCursor(stage: String) {
        val b = bridge()
        b.setCursorBlinkEnabled(false) // blink-off phase would hide the block
        Thread.sleep(150)
        val density =
            InstrumentationRegistry.getInstrumentation().targetContext.resources.displayMetrics.density
        val packed = b.cursorViewportPacked()
        val row = if (packed >= 0) (packed shr 32).toInt() else -1
        val col = if (packed >= 0) (packed and 0xffffffffL).toInt() else -1
        val bbox = loneBlockBbox()
        val cellCol = if (minXof(bbox) >= 0) minXof(bbox) / (b.getCellWidth() * density) else -1f
        val cellRow =
            if (minYof(bbox) >= 0) {
                // Approximate: surface top ≈ status bar bottom (~150px on this
                // device profile); cell height in physical px from CSS metrics.
                (minYof(bbox) - 150) / (b.getCellHeight() * density)
            } else {
                -1f
            }
        android.util.Log.i(
            "DiagGrid",
            "$stage: renderCursor=(row=$row,col=$col) $bbox blockCell≈(row=$cellRow,col=$cellCol) density=$density",
        )
    }

    private fun minXof(bbox: String): Int = Regex("""x\[(-?\d+),""").find(bbox)?.groupValues?.get(1)?.toInt() ?: -1

    private fun minYof(bbox: String): Int = Regex("""y\[(-?\d+),""").find(bbox)?.groupValues?.get(1)?.toInt() ?: -1

    @Test
    fun correlateCursorDataWithPixels() {
        // Boot ground truth: wait for the prompt, then compare the VT
        // cursor with the rendered block BEFORE any input.
        val b = bridge()
        val deadline = System.currentTimeMillis() + 20_000
        while (System.currentTimeMillis() < deadline) {
            val text = b.getTerminalText()
            if (!text.isNullOrBlank()) break
            Thread.sleep(200)
        }
        Thread.sleep(1_500)
        val packed = b.cursorViewportPacked()
        val row0 = if (packed >= 0) (packed shr 32).toInt() else -1
        val col0 = if (packed >= 0) (packed and 0xffffffffL).toInt() else -1
        android.util.Log.i(
            "DiagGrid",
            "BOOT: renderCursor=(row=$row0,col=$col0) text0=<${b.getTerminalText()?.lineSequence()?.firstOrNull()}> sb0=<${b.scrollbackLine(0)}> sbLen=${b.scrollbackLength()}",
        )
        logRowProfile("BOOT")
        logCursor("T0-initial")
        bridge().writeToPty("abc".toByteArray())
        Thread.sleep(900)
        logCursor("T1-after-abc")
        logRowProfile("T1")
        bridge().writeToPty("def".toByteArray())
        Thread.sleep(900)
        logCursor("T2-after-def")
        bridge().writeToPty("\n".toByteArray())
        Thread.sleep(900)
        logCursor("T3-after-enter")
        logRowProfile("T3")
    }
}
