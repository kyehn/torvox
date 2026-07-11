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
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.torvox.MainActivity
import org.junit.After
import org.junit.Assert
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import kotlin.math.abs
import kotlin.math.sqrt

@RunWith(AndroidJUnit4::class)
class InlineScreenshotVerificationTest {
    private var scenario: ActivityScenario<MainActivity>? = null
    private var tv: TextureView? = null

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
    }

    @Before
    fun setUp() {
        scenario = ActivityScenario.launch(MainActivity::class.java)
        scenario!!.onActivity { activity ->
            tv = findTextureView(activity.window.decorView)
        }
        Assert.assertNotNull("TextureView not found", tv)
        waitForBridgeReady()
    }

    @After
    fun tearDown() {
        scenario?.close()
    }

    private fun waitForBridgeReady(timeoutMs: Long = 60_000) {
        val start = System.currentTimeMillis()
        var ok = false
        while (System.currentTimeMillis() - start < timeoutMs) {
            scenario!!.onActivity { activity ->
                if (activity is MainActivity) {
                    ok = activity.torvoxRuntime?.bridge() != null
                }
            }
            if (ok) {
                Thread.sleep(2000)
                return
            }
            Thread.sleep(500)
        }
        Assert.fail("Bridge not ready after ${timeoutMs}ms")
    }

    private fun longPressOn(
        x: Float,
        y: Float,
    ) {
        val dt = android.os.SystemClock.uptimeMillis()
        tv!!.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0),
        )
        Thread.sleep(800)
        tv!!.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + 800, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0),
        )
        Thread.sleep(50)
        tv!!.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + 850, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0),
        )
    }

    private fun capture(): Bitmap? {
        Thread.sleep(500)
        val holder = arrayOfNulls<Bitmap>(1)
        scenario!!.onActivity { activity ->
            val view = activity.window.decorView
            if (view.width <= 0 || view.height <= 0) return@onActivity
            val bmp = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bmp)
            view.draw(canvas)
            holder[0] = bmp
        }
        return holder[0]
    }

    private fun writeToPty(data: String) {
        var bridge: io.torvox.bridge.TorvoxBridge? = null
        scenario!!.onActivity { activity ->
            bridge = (activity as? MainActivity)?.torvoxRuntime?.bridge()
        }
        bridge?.writeToPty(data.toByteArray())
    }

    private data class Blob(
        val x1: Int,
        val y1: Int,
        val x2: Int,
        val y2: Int,
    ) {
        val cx get() = (x1 + x2) / 2
        val cy get() = (y1 + y2) / 2
        val w get() = x2 - x1 + 1
        val h get() = y2 - y1 + 1
    }

    private fun findBlobs(
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
                var diff = 0
                diff += abs(Color.red(before.getPixel(x, y)) - Color.red(after.getPixel(x, y)))
                diff += abs(Color.green(before.getPixel(x, y)) - Color.green(after.getPixel(x, y)))
                diff += abs(Color.blue(before.getPixel(x, y)) - Color.blue(after.getPixel(x, y)))
                changed[y][x] = diff > threshold
            }
        }
        val visited = Array(h) { BooleanArray(w) }
        val blobs = mutableListOf<Blob>()
        for (y in 0 until h) {
            for (x in 0 until w) {
                if (!changed[y][x] || visited[y][x]) continue
                var x1 = x
                var x2 = x
                var y1 = y
                var y2 = y
                val stack = ArrayDeque<Pair<Int, Int>>()
                stack.addLast(x to y)
                visited[y][x] = true
                while (stack.isNotEmpty()) {
                    val (cx, cy) = stack.removeLast()
                    x1 = minOf(x1, cx)
                    x2 = maxOf(x2, cx)
                    y1 = minOf(y1, cy)
                    y2 = maxOf(y2, cy)
                    for (dx in -1..1) {
                        for (dy in -1..1) {
                            val nx = cx + dx
                            val ny = cy + dy
                            if (nx in 0 until w && ny in 0 until h && changed[ny][nx] && !visited[ny][nx]) {
                                visited[ny][nx] = true
                                stack.addLast(nx to ny)
                            }
                        }
                    }
                }
                val bw = x2 - x1 + 1
                val bh = y2 - y1 + 1
                if (bw >= minSize && bh >= minSize) blobs.add(Blob(x1, y1, x2, y2))
            }
        }
        return blobs
    }

    @Test
    fun verifyPasteMenuAppearsOnBlankSwipe() {
        Log.i("InlineVerif", "=== Verify Paste Menu ===")
        writeToPty("echo 'test content'\n")
        Thread.sleep(3000)

        val w = tv!!.width
        val h = tv!!.height

        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(ClipData.newPlainText("test", "pasted data"))

        val lpX = w * 0.5f
        val lpY = h * 0.85f
        Log.i("InlineVerif", "Blank long-press at ($lpX, $lpY)")

        val base = capture()
        Assert.assertNotNull("Baseline null", base)
        Log.i("InlineVerif", "Baseline: ${base!!.width}x${base!!.height}")

        longPressOn(lpX, lpY)
        Thread.sleep(2000)

        val pasteScr = capture()
        Assert.assertNotNull("Paste screenshot null", pasteScr)
        Log.i("InlineVerif", "Paste: ${pasteScr!!.width}x${pasteScr!!.height}")

        val blobs = findBlobs(base, pasteScr, minSize = 20)
        val totalPx = blobs.sumOf { it.w * it.h }
        Log.i("InlineVerif", "Changed pixels: $totalPx (${blobs.size} blobs)")

        val largeBlobs = blobs.filter { it.w > 100 || it.h > 60 }
        Log.i("InlineVerif", "Large blobs (>100px wide or >60px tall): ${largeBlobs.size}")
        largeBlobs.forEachIndexed { i, b ->
            Log.i("InlineVerif", "  Blob $i: (${b.x1},${b.y1})-(${b.x2},${b.y2}) ${b.w}x${b.h}")
        }

        Assert.assertTrue(
            "No paste toolbar visible (expected large blob)",
            largeBlobs.isNotEmpty(),
        )

        val nearBottom = largeBlobs.any { abs(it.cy - lpY.toInt()) < h / 4 }
        Assert.assertTrue("Toolbar not near long-press position", nearBottom)

        saveScreenshot(base, "paste-baseline")
        saveScreenshot(pasteScr, "paste-toolbar")
        Log.i("InlineVerif", "=== Paste Menu PASSED ===")
    }

    private fun saveScreenshot(
        bitmap: Bitmap,
        name: String,
    ) {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(ctx.filesDir, "screenshots")
        dir.mkdirs()
        val file = File(dir, "inline-$name.png")
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        Log.i("InlineVerif", "Saved: ${file.absolutePath} (${file.length()}B)")
        val extDir = ctx.getExternalFilesDir("Pictures")
        if (extDir != null) {
            extDir.mkdirs()
            File(extDir, "inline-$name.png").outputStream().use {
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
            }
        }
    }
}
