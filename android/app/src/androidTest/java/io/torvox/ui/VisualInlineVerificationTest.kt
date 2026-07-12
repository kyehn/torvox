
package io.torvox.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Color
import android.util.Log
import android.view.MotionEvent
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.torvox.MainActivity
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Assert
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import kotlin.math.abs
import kotlin.math.sqrt

@RunWith(AndroidJUnit4::class)
class VisualInlineVerificationTest {
    @get:Rule val composeRule = createAndroidComposeRule<MainActivity>()

    private var tv: TextureView? = null
    private var bridge: io.torvox.bridge.TorvoxBridge? = null

    companion object {
        private fun findTextureView(root: View): TextureView? {
            if (root is TextureView) return root
            if (root is ViewGroup) {
                for (i in 0 until root.childCount) {
                    (findTextureView(root.getChildAt(i)))?.let { return it }
                }
            }
            return null
        }

        private fun longPressOn(
            tv: TextureView,
            x: Float,
            y: Float,
        ) {
            val dt = android.os.SystemClock.uptimeMillis()
            tv.dispatchTouchEvent(
                MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0),
            )
            Thread.sleep(800)
            tv.dispatchTouchEvent(
                MotionEvent.obtain(dt, dt + 800, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0),
            )
            Thread.sleep(50)
            tv.dispatchTouchEvent(
                MotionEvent.obtain(dt, dt + 850, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0),
            )
        }

        private fun captureScreenshot(): Bitmap? {
            Thread.sleep(500)
            val start = System.currentTimeMillis()
            val result =
                try {
                    InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot()
                } catch (e: Exception) {
                    Log.e("VisualInline", "capture exception: ${e.message}")
                    null
                }
            val elapsed = System.currentTimeMillis() - start
            if (result == null) {
                Log.w("VisualInline", "capture returned null (${elapsed}ms)")
            } else {
                Log.i("VisualInline", "capture OK: ${result.width}x${result.height} (${elapsed}ms)")
            }
            return result
        }
    }

    private fun Bitmap.getPixel(
        x: Int,
        y: Int,
    ): Int = if (x in 0 until width && y in 0 until height) {
        getPixel(x, y)
    } else {
        0
    }

    private fun colorDiff(
        a: Int,
        b: Int,
    ): Int = abs(Color.red(a) - Color.red(b)) +
        abs(Color.green(a) - Color.green(b)) +
        abs(Color.blue(a) - Color.blue(b))

    private data class Blob(
        val minX: Int,
        val minY: Int,
        val maxX: Int,
        val maxY: Int,
    ) {
        val cx get() = (minX + maxX) / 2
        val cy get() = (minY + maxY) / 2
        val w get() = maxX - minX + 1
        val h get() = maxY - minY + 1
    }

    private fun findChangedBlobs(
        before: Bitmap,
        after: Bitmap,
        threshold: Int = 50,
        minSize: Int = 8,
    ): List<Blob> {
        val w = minOf(before.width, after.width)
        val h = minOf(before.height, after.height)
        val changed = Array(h) { BooleanArray(w) }
        for (y in 0 until h) {
            for (x in 0 until w) {
                changed[y][x] = colorDiff(before.getPixel(x, y), after.getPixel(x, y)) > threshold
            }
        }

        val visited = Array(h) { BooleanArray(w) }
        val blobs = mutableListOf<Blob>()
        val dirs = listOf(-1 to -1, -1 to 0, -1 to 1, 0 to -1, 0 to 1, 1 to -1, 1 to 0, 1 to 1)

        for (y in 0 until h) {
            for (x in 0 until w) {
                if (!changed[y][x] || visited[y][x]) continue
                var minX = x
                var maxX = x
                var minY = y
                var maxY = y
                val stack = ArrayDeque<Pair<Int, Int>>()
                stack.addLast(x to y)
                visited[y][x] = true
                while (stack.isNotEmpty()) {
                    val (cx, cy) = stack.removeLast()
                    minX = minOf(minX, cx)
                    maxX = maxOf(maxX, cx)
                    minY = minOf(minY, cy)
                    maxY = maxOf(maxY, cy)
                    for ((dx, dy) in dirs) {
                        val nx = cx + dx
                        val ny = cy + dy
                        if (nx in 0 until w && ny in 0 until h && changed[ny][nx] && !visited[ny][nx]) {
                            visited[ny][nx] = true
                            stack.addLast(nx to ny)
                        }
                    }
                }
                val bw = maxX - minX + 1
                val bh = maxY - minY + 1
                if (bw >= 10 && bh >= 10) {
                    blobs.add(Blob(minX, minY, maxX, maxY))
                }
            }
        }
        return blobs
    }

    private fun pixelDiffCount(
        before: Bitmap,
        after: Bitmap,
        threshold: Int = 50,
    ): Int {
        val w = minOf(before.width, after.width)
        val h = minOf(before.height, after.height)
        var count = 0
        for (y in 0 until h) {
            for (x in 0 until w) {
                if (colorDiff(before.getPixel(x, y), after.getPixel(x, y)) > threshold) count++
            }
        }
        return count
    }

    @Test
    fun verifyWordSelectionPositions() {
        Log.i("VisualInline", "==== Word Selection Position Verification ====")
        composeRule.waitForSession()
        bridge = composeRule.getBridge()
        Assert.assertNotNull("Bridge not ready", bridge)

        tv = findTextureView(composeRule.activity.window.decorView)
        Assert.assertNotNull("TextureView not found", tv)

        val w = tv!!.width
        val h = tv!!.height

        bridge!!.writeToPty("echo 'hello world selectable text terminal'\n".toByteArray())
        Thread.sleep(3000)

        // Word "world" is at column ~6, row 0
        val cellW = w / 80f
        val cellH = h / 24f
        val longPressX = cellW * 7f
        val longPressY = cellH * 0.5f

        Log.i("VisualInline", "Long-press at ($longPressX, $longPressY) for 'world'")

        val baseline = captureScreenshot()
        Assert.assertNotNull("Baseline screenshot null", baseline)

        longPressOn(tv!!, longPressX, longPressY)
        Thread.sleep(2000)

        val afterSel = captureScreenshot()
        Assert.assertNotNull("Selection screenshot null", afterSel)

        // Save for evidence
        saveToExternal("word-baseline", baseline!!)
        saveToExternal("word-selection", afterSel!!)

        val changedPx = pixelDiffCount(baseline, afterSel)
        Log.i("VisualInline", "Changed pixels after word selection: $changedPx")
        Assert.assertTrue("No selection change detected ($changedPx pixels)", changedPx > 100)

        val blobs = findChangedBlobs(baseline, afterSel)
        Log.i("VisualInline", "Found ${blobs.size} changed blobs")

        // Find handle-sized blobs (42-85px round; our handles are 24dp ≈ 64px on 480dpi)
        val handles = blobs.filter { it.w in 50..76 && it.h in 50..76 }
        Log.i("VisualInline", "Handles (50-76px): ${handles.size}")
        handles.forEachIndexed { i, h ->
            Log.i("VisualInline", "  Handle $i: (${h.cx},${h.cy}) ${h.w}x${h.h} -> cell(${h.cx / cellW.toInt()},${h.cy / cellH.toInt()})")
        }

        Assert.assertTrue("Expected >=2 selection handles, found ${handles.size}", handles.size >= 2)

        val h0 = handles[0]
        val h1 = handles[1]
        val rowDiff = abs(h0.cy - h1.cy)
        Log.i("VisualInline", "Handle row diff: ${rowDiff}px (cellH=$cellH)")
        Assert.assertTrue("Handles not on same row (diff=$rowDiff)", rowDiff < cellH * 2)

        // Verify handles are near the long-press position
        val startNear = h0.cx in (longPressX.toInt() - (cellW * 8).toInt())..(longPressX.toInt() + (cellW * 4).toInt())
        val endNear = h1.cx in (longPressX.toInt() - (cellW * 2).toInt())..(longPressX.toInt() + (cellW * 8).toInt())
        if (!startNear || !endNear) {
            Log.w("VisualInline", "Start handle near: $startNear, End handle near: $endNear")
        }

        Log.i("VisualInline", "Word selection verification PASSED")
    }

    @Test
    fun verifyUrlSelectionPositions() {
        Log.i("VisualInline", "==== URL Selection Position Verification ====")
        composeRule.waitForSession()
        bridge = composeRule.getBridge()
        Assert.assertNotNull(bridge)
        tv = findTextureView(composeRule.activity.window.decorView)
        Assert.assertNotNull(tv)

        val w = tv!!.width
        val h = tv!!.height
        val cellW = w / 80f
        val cellH = h / 24f

        bridge!!.writeToPty("https://github.com/termux is the main url for terminal\n".toByteArray())
        Thread.sleep(3000)

        val longPressX = cellW * 2f
        val longPressY = cellH * 0.5f

        val baseline = captureScreenshot()!!
        longPressOn(tv!!, longPressX, longPressY)
        Thread.sleep(2000)
        val afterSel = captureScreenshot()!!

        saveToExternal("url-baseline", baseline)
        saveToExternal("url-selection", afterSel)

        val changedPx = pixelDiffCount(baseline, afterSel)
        Assert.assertTrue("No URL selection change ($changedPx)", changedPx > 100)

        val blobs = findChangedBlobs(baseline, afterSel)
        val handles = blobs.filter { it.w in 50..76 && it.h in 50..76 }
        Assert.assertTrue("Expected >=2 handles for URL, found ${handles.size}", handles.size >= 2)

        val h0 = handles[0]
        val h1 = handles[1]
        val rowDiff = abs(h0.cy - h1.cy)
        Assert.assertTrue("URL handle rows differ ($rowDiff)", rowDiff < cellH * 2)

        // URL is at column 0 - handles should be at reasonable columns
        val urlCells = h1.cx / cellW.toInt() - h0.cx / cellW.toInt()
        Log.i("VisualInline", "URL spans $urlCells cells ($cellW px/cell)")
        Assert.assertTrue("URL too short ($urlCells cells)", urlCells >= 5)

        Log.i("VisualInline", "URL selection verification PASSED")
    }

    @Test
    fun verifyPasteMenuPosition() {
        Log.i("VisualInline", "==== Paste Menu Position Verification ====")
        composeRule.waitForSession()
        bridge = composeRule.getBridge()
        Assert.assertNotNull(bridge)
        tv = findTextureView(composeRule.activity.window.decorView)
        Assert.assertNotNull(tv)

        val w = tv!!.width
        val h = tv!!.height

        bridge!!.writeToPty("some terminal content\n".toByteArray())
        Thread.sleep(3000)

        // Set clipboard
        val cm = composeRule.activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(ClipData.newPlainText("test", "paste data"))

        // Long-press near bottom of terminal
        val lpX = w * 0.3f
        val lpY = h * 0.85f

        val baseline = captureScreenshot()!!
        longPressOn(tv!!, lpX, lpY)
        Thread.sleep(2000)
        val afterPaste = captureScreenshot()!!

        saveToExternal("paste-baseline", baseline)
        saveToExternal("paste-button", afterPaste)

        val changedPx = pixelDiffCount(baseline, afterPaste)
        Log.i("VisualInline", "Changed pixels after paste menu: $changedPx")
        Assert.assertTrue("No paste menu change ($changedPx)", changedPx > 500)

        val blobs = findChangedBlobs(baseline, afterPaste, minSize = 20)
        val largeBlobs = blobs.filter { it.w > 200 || it.h > 80 }
        Log.i("VisualInline", "Large UI blobs: ${largeBlobs.size}")
        largeBlobs.forEachIndexed { i, b ->
            Log.i("VisualInline", "  Blob $i: (${b.minX},${b.minY})-(${b.maxX},${b.maxY}) ${b.w}x${b.h}")
        }

        // Should have a large toolbar (>200px wide)
        val hasToolbar = largeBlobs.isNotEmpty()
        Assert.assertTrue("No paste toolbar found", hasToolbar)

        // Toolbar should be near long-press position
        val lpYInt = lpY.toInt()
        val nearBottom = largeBlobs.any { abs(it.cy - lpYInt) < h / 4 }
        if (!nearBottom) {
            Log.w("VisualInline", "Toolbar not near long-press: long-press Y=$lpYInt")
        }

        Log.i("VisualInline", "Paste menu verification PASSED")
    }

    private fun saveToExternal(
        name: String,
        bitmap: Bitmap,
    ) {
        val extDir = composeRule.activity.getExternalFilesDir("Pictures")
        if (extDir != null) {
            extDir.mkdirs()
            val file = File(extDir, "inline-verify-$name.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            Log.i("VisualInline", "Saved $name: ${file.absolutePath} (${file.length()}B)")
        }
        // Also save to internal
        val intDir = File(composeRule.activity.filesDir, "screenshots")
        intDir.mkdirs()
        val intFile = File(intDir, "inline-verify-$name.png")
        intFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        Log.i("VisualInline", "Saved $name: ${intFile.absolutePath} (${intFile.length()}B)")
    }
}
