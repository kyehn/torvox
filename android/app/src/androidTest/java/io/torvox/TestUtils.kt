package io.torvox

import android.app.Activity
import android.graphics.Bitmap
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.platform.app.InstrumentationRegistry
import io.torvox.bridge.TorvoxBridge
import org.junit.Assert.assertTrue
import java.io.File
import kotlin.math.abs

// ── Data model ──────────────────────────────────────

data class PixelFrame(
    val pixels: IntArray,
    val width: Int,
    val height: Int,
)

// ── Session helpers ─────────────────────────────────

fun AndroidComposeTestRule<*, *>.waitForSession(timeoutMs: Long = 60_000) {
    System.setProperty("torvox.test.minSurface", "true")
    // Use the standard assertion approach (same as search steps) instead of
    // allNodes + fetchSemanticsNodes, which may fail in merged-tree scenarios
    waitUntil(timeoutMillis = timeoutMs) {
        try {
            onNodeWithTag("TerminalScreen").assertIsDisplayed()
            true
        } catch (e: AssertionError) {
            false
        } catch (e: Exception) {
            false
        }
    }
}

fun AndroidComposeTestRule<*, *>.getBridge(): TorvoxBridge? {
    var bridge: TorvoxBridge? = null
    val rule = activityRule as ActivityScenarioRule<*>
    val deadlineMs = System.currentTimeMillis() + 15_000
    while (bridge == null && System.currentTimeMillis() < deadlineMs) {
        Thread.sleep(100)
        rule.scenario.onActivity { activity: android.app.Activity ->
            bridge = (activity as MainActivity).torvoxRuntime.bridge()
        }
    }
    return bridge
}

fun AndroidComposeTestRule<*, *>.openDrawer() {
    waitForIdle()
    onNodeWithTag("Key_DRAWER").performClick()
    waitForIdle()
}

fun AndroidComposeTestRule<*, *>.openSettings() {
    openDrawer()
    onNodeWithTag("SettingsButton").performClick()
    waitForIdle()
}

// ── GPU frame helpers ───────────────────────────────

fun getDisplayWidth(): Int {
    val context = InstrumentationRegistry.getInstrumentation().targetContext
    val windowManager = context.getSystemService(android.view.WindowManager::class.java)
    return windowManager.currentWindowMetrics.bounds.width()
}

fun decodeRgbaToPixels(file: File): PixelFrame {
    val bytes = file.readBytes()
    val width = u32FromLe(bytes, 0)
    val pixelBytes = bytes.drop(4)
    val totalPixels = pixelBytes.size / 4
    val height = totalPixels / width
    val pixels = IntArray(width * height)
    for (i in 0 until width * height) {
        val r = pixelBytes[i * 4].toInt() and 0xFF
        val g = pixelBytes[i * 4 + 1].toInt() and 0xFF
        val b = pixelBytes[i * 4 + 2].toInt() and 0xFF
        val a = pixelBytes[i * 4 + 3].toInt() and 0xFF
        pixels[i] = (a shl 24) or (r shl 16) or (g shl 8) or b
    }
    return PixelFrame(pixels, width, height)
}

fun decodeRgbaToBitmap(file: File): Bitmap {
    val frame = decodeRgbaToPixels(file)
    return Bitmap.createBitmap(frame.pixels, frame.width, frame.height, Bitmap.Config.ARGB_8888)
}

fun saveAsPng(
    frame: PixelFrame,
    to: File,
) {
    val bitmap = Bitmap.createBitmap(frame.pixels, frame.width, frame.height, Bitmap.Config.ARGB_8888)
    to.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    bitmap.recycle()
}

// ── Pixel analysis helpers ──────────────────────────

fun analyzeNonBlackRatio(bitmap: Bitmap): Double {
    val width = bitmap.width
    val height = bitmap.height
    val pixels = IntArray(width * height)
    bitmap.getPixels(pixels, 0, width, 0, 0, width, height)
    var nonBlack = 0L
    for (pixel in pixels) {
        val r = (pixel shr 16) and 0xFF
        val g = (pixel shr 8) and 0xFF
        val b = pixel and 0xFF
        if (r > 15 || g > 15 || b > 15) nonBlack++
    }
    return nonBlack.toDouble() / pixels.size.toDouble()
}

fun analyzeNonBlackRatio(frame: PixelFrame): Double {
    var nonBlack = 0L
    for (pixel in frame.pixels) {
        val r = (pixel shr 16) and 0xFF
        val g = (pixel shr 8) and 0xFF
        val b = pixel and 0xFF
        if (r > 10 || g > 10 || b > 10) nonBlack++
    }
    return nonBlack.toDouble() / frame.pixels.size.toDouble()
}

// ── Integer-division-safe cell geometry ────────────

fun cellX(
    col: Int,
    frameWidth: Int,
    gridCols: Int,
): Int = (col * frameWidth) / gridCols

fun cellRight(
    col: Int,
    frameWidth: Int,
    gridCols: Int,
): Int = ((col + 1) * frameWidth) / gridCols

fun cellY(
    row: Int,
    frameHeight: Int,
    gridRows: Int,
): Int = (row * frameHeight) / gridRows

fun cellBottom(
    row: Int,
    frameHeight: Int,
    gridRows: Int,
): Int = ((row + 1) * frameHeight) / gridRows

fun cellWidth(
    col: Int,
    frameWidth: Int,
    gridCols: Int,
): Int = cellRight(col, frameWidth, gridCols) - cellX(col, frameWidth, gridCols)

fun cellHeight(
    row: Int,
    frameHeight: Int,
    gridRows: Int,
): Int = cellBottom(row, frameHeight, gridRows) - cellY(row, frameHeight, gridRows)

// ── Pixel matching core ─────────────────────────────

fun extractCell(
    frame: PixelFrame,
    col: Int,
    row: Int,
    gridCols: Int,
    gridRows: Int,
): IntArray {
    val fw = frame.width
    val fh = frame.height
    val left = cellX(col, fw, gridCols)
    val right = cellRight(col, fw, gridCols)
    val top = cellY(row, fh, gridRows)
    val bot = cellBottom(row, fh, gridRows)
    val cw = right - left
    val ch = bot - top
    val region = IntArray(cw * ch)
    for (y in 0 until ch) {
        for (x in 0 until cw) {
            val frameIdx = (top + y) * fw + (left + x)
            region[y * cw + x] = frame.pixels.getOrElse(frameIdx) { 0 }
        }
    }
    return region
}

fun buildTemplate(
    frame: PixelFrame,
    col: Int,
    row: Int,
    gridCols: Int,
    gridRows: Int,
): IntArray = extractCell(frame, col, row, gridCols, gridRows)

fun matchConfidence(
    actual: IntArray,
    template: IntArray,
): Double {
    val len = minOf(actual.size, template.size)
    if (len == 0) return 0.0
    var matching = 0
    for (i in 0 until len) {
        val a = actual[i]
        val b = template[i]
        val dr = abs(((a shr 16) and 0xFF) - ((b shr 16) and 0xFF))
        val dg = abs(((a shr 8) and 0xFF) - ((b shr 8) and 0xFF))
        val db = abs((a and 0xFF) - (b and 0xFF))
        if (dr + dg + db < 60) matching++
    }
    return matching.toDouble() / len.toDouble()
}

fun rowProfile(frame: PixelFrame): IntArray = IntArray(frame.height) { row ->
    var count = 0
    for (col in 0 until frame.width) {
        val p = frame.pixels[row * frame.width + col]
        val r = (p shr 16) and 0xFF
        val g = (p shr 8) and 0xFF
        val b = p and 0xFF
        if (r > 10 || g > 10 || b > 10) count++
    }
    count
}

fun lastGridRowsDensity(
    profile: IntArray,
    frameWidth: Int,
    frameHeight: Int,
    gridRows: Int,
    checkRows: Int,
): Double {
    var maxDensity = 0.0
    for (r in (gridRows - checkRows).coerceAtLeast(0) until gridRows) {
        val top = cellY(r, frameHeight, gridRows)
        val bot = cellBottom(r, frameHeight, gridRows)
        var nonBlack = 0L
        var total = 0L
        for (pr in top until bot) {
            nonBlack += profile[pr]
            total += frameWidth
        }
        val density = if (total > 0) nonBlack.toDouble() / total.toDouble() else 0.0
        if (density > maxDensity) maxDensity = density
    }
    return maxDensity
}

// ── Gesture helpers ─────────────────────────────────

fun injectLongPress(
    activity: Activity,
    view: View,
    x: Float,
    y: Float,
) {
    val dt = SystemClock.uptimeMillis()
    view.dispatchTouchEvent(MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0))
    Thread.sleep(800)
    view.dispatchTouchEvent(MotionEvent.obtain(dt, dt + 800, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0))
    view.dispatchTouchEvent(MotionEvent.obtain(dt, dt + 850, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0))
}

fun injectTap(
    activity: Activity,
    view: View,
    x: Float,
    y: Float,
) {
    val dt = SystemClock.uptimeMillis()
    view.dispatchTouchEvent(MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0))
    view.dispatchTouchEvent(MotionEvent.obtain(dt, dt + 50, MotionEvent.ACTION_UP, x, y, 0))
}

fun injectDoubleTap(
    activity: Activity,
    view: View,
    x: Float,
    y: Float,
) {
    injectTap(activity, view, x, y)
    Thread.sleep(150)
    injectTap(activity, view, x, y)
}

fun injectTripleTap(
    activity: Activity,
    view: View,
    x: Float,
    y: Float,
) {
    injectTap(activity, view, x, y)
    Thread.sleep(150)
    injectTap(activity, view, x, y)
    Thread.sleep(150)
    injectTap(activity, view, x, y)
}

// ── Selection assertion helpers ─────────────────────

fun assertSelectionMatches(
    templateFrame: PixelFrame,
    selectionFrame: PixelFrame,
    gridCols: Int,
    gridRows: Int,
    row: Int,
    colRange: IntRange,
    maxConfidence: Double = 0.5,
    minConfidenceUnselected: Double = 0.9,
) {
    for (col in colRange) {
        val actual = extractCell(selectionFrame, col, row, gridCols, gridRows)
        val tmpl = extractCell(templateFrame, col, row, gridCols, gridRows)
        val c = matchConfidence(actual, tmpl)
        assertTrue("Selected cell ($row,$col): confidence $c > $maxConfidence", c <= maxConfidence)
    }
    val checkLeft = (colRange.first - 1).coerceAtLeast(0)
    if (checkLeft < colRange.first) {
        val c =
            matchConfidence(
                extractCell(selectionFrame, checkLeft, row, gridCols, gridRows),
                extractCell(templateFrame, checkLeft, row, gridCols, gridRows),
            )
        assertTrue("Unselected left ($checkLeft,$row): $c < $minConfidenceUnselected", c >= minConfidenceUnselected)
    }
    val checkRight = (colRange.last + 1).coerceAtMost(gridCols - 1)
    if (checkRight > colRange.last && checkRight < gridCols) {
        val c =
            matchConfidence(
                extractCell(selectionFrame, checkRight, row, gridCols, gridRows),
                extractCell(templateFrame, checkRight, row, gridCols, gridRows),
            )
        assertTrue("Unselected right ($checkRight,$row): $c < $minConfidenceUnselected", c >= minConfidenceUnselected)
    }
}

fun findTerminalSurface(activity: Activity): View {
    val content = activity.findViewById<View>(android.R.id.content) as ViewGroup
    return content.findViewWithTag<View>("TerminalSurfaceView")
        ?: run {
            fun traverse(group: ViewGroup): View? {
                for (i in 0 until group.childCount) {
                    val child = group.getChildAt(i)
                    if (child is android.view.TextureView) return child
                    if (child is ViewGroup) {
                        val result = traverse(child)
                        if (result != null) return result
                    }
                }
                return null
            }
            traverse(content) ?: content
        }
}

// ── Private helpers ─────────────────────────────────

private fun u32FromLe(
    bytes: ByteArray,
    offset: Int,
): Int = (bytes[offset].toInt() and 0xFF) or
    ((bytes[offset + 1].toInt() and 0xFF) shl 8) or
    ((bytes[offset + 2].toInt() and 0xFF) shl 16) or
    ((bytes[offset + 3].toInt() and 0xFF) shl 24)
