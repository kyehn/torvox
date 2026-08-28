package terminal.emulator

import android.graphics.Bitmap
import android.os.SystemClock
import android.util.Log
import androidx.test.uiautomator.UiDevice
import java.io.File

/**
 * Shared helpers for the UX-quantified verification suite.
 *
 * Every test in this suite follows the same contract demanded by the review: a previously
 * user-reported behavior is only "fixed" when an automated, MEASURABLE assertion passes on the
 * emulator — never on code reading alone. Each assertion also logs its measured value as a
 * `UX_METRIC key=value` logcat line so trends are trackable across runs (grep -e UX_METRIC).
 */
object UxTestUtils {
    private const val TAG = "UX_METRIC"

    /**
     * Inject a drag gesture as a real event stream into [view] (the house TestUtils
     * dispatchTouchEvent pattern): ACTION_DOWN at ([x0], [y0]), [steps] evenly spaced ACTION_MOVEs
     * with [stepDelayMs] pacing, then ACTION_UP at ([x1], [y1]). Unlike UiDevice.swipe this exposes
     * the per-step pacing needed for live-highlight sampling.
     */
    fun injectDrag(
        view: android.view.View,
        x0: Float,
        y0: Float,
        x1: Float,
        y1: Float,
        steps: Int = 6,
        stepDelayMs: Long = 120,
    ) {
        val downTime = SystemClock.uptimeMillis()
        fun post(action: Int, x: Float, y: Float) {
            val t = SystemClock.uptimeMillis()
            view.post {
                val event = android.view.MotionEvent.obtain(downTime, t, action, x, y, 0)
                try {
                    view.dispatchTouchEvent(event)
                } finally {
                    event.recycle()
                }
            }
        }
        post(android.view.MotionEvent.ACTION_DOWN, x0, y0)
        try {
            for (step in 1..steps) {
                Thread.sleep(stepDelayMs)
                val frac = step.toFloat() / steps
                post(android.view.MotionEvent.ACTION_MOVE, x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
            }
            Thread.sleep(stepDelayMs)
        } finally {
            post(android.view.MotionEvent.ACTION_UP, x1, y1)
            // Let the main thread drain UP before the caller samples anything.
            Thread.sleep(150)
        }
    }

    /** Capture the full device screen as a bitmap via UiDevice. */
    fun screenshot(device: UiDevice): Bitmap {
        val file = File.createTempFile("uxquant", ".png")
        device.takeScreenshot(file)
        val bitmap = android.graphics.BitmapFactory.decodeFile(file.path)
        file.delete()
        check(bitmap != null) { "takeScreenshot produced an undecodable file" }
        return bitmap
    }

    /**
     * Count pixels that differ materially between two same-sized captures. Materially = any channel
     * delta > [tolerance], which absorbs codec noise while still counting real content changes.
     */
    fun changedPixelCount(
        before: Bitmap,
        after: Bitmap,
        tolerance: Int = 12,
    ): Int {
        require(before.width == after.width && before.height == after.height) {
            "capture size mismatch: ${before.width}x${before.height} vs ${after.width}x${after.height}"
        }
        val w = before.width
        val h = before.height
        val a = IntArray(w * h)
        val b = IntArray(w * h)
        before.getPixels(a, 0, w, 0, 0, w, h)
        after.getPixels(b, 0, w, 0, 0, w, h)
        var changed = 0
        for (i in a.indices) {
            val pa = a[i]
            val pb = b[i]
            if (
                kotlin.math.abs((pa shr 16 and 0xFF) - (pb shr 16 and 0xFF)) > tolerance ||
                kotlin.math.abs((pa shr 8 and 0xFF) - (pb shr 8 and 0xFF)) > tolerance ||
                kotlin.math.abs((pa and 0xFF) - (pb and 0xFF)) > tolerance
            ) {
                changed++
            }
        }
        return changed
    }

    /**
     * Bounding box of pixels differing between [before] and [after], or null when nothing changed
     * beyond [tolerance]. Used by the cursor geometry test: diffing cursor-at-col-A vs
     * cursor-at-col-B isolates exactly the two cursor quads — their bbox IS the measurable cursor
     * geometry, independent of theme colors.
     */
    fun changedBoundingBox(
        before: Bitmap,
        after: Bitmap,
        tolerance: Int = 12,
    ): Rect? {
        require(before.width == after.width && before.height == after.height) {
            "capture size mismatch: ${before.width}x${before.height} vs ${after.width}x${after.height}"
        }
        val w = before.width
        val h = before.height
        val a = IntArray(w * h)
        val b = IntArray(w * h)
        before.getPixels(a, 0, w, 0, 0, w, h)
        after.getPixels(b, 0, w, 0, 0, w, h)
        var minX = Int.MAX_VALUE
        var minY = Int.MAX_VALUE
        var maxX = Int.MIN_VALUE
        var maxY = Int.MIN_VALUE
        for (y in 0 until h) {
            for (x in 0 until w) {
                val i = y * w + x
                val pa = a[i]
                val pb = b[i]
                if (
                    kotlin.math.abs((pa shr 16 and 0xFF) - (pb shr 16 and 0xFF)) > tolerance ||
                    kotlin.math.abs((pa shr 8 and 0xFF) - (pb shr 8 and 0xFF)) > tolerance ||
                    kotlin.math.abs((pa and 0xFF) - (pb and 0xFF)) > tolerance
                ) {
                    if (x < minX) minX = x
                    if (x > maxX) maxX = x
                    if (y < minY) minY = y
                    if (y > maxY) maxY = y
                }
            }
        }
        return if (maxX < minX) null else Rect(minX, minY, maxX, maxY)
    }

    /**
     * Poll [predicate] until it turns true or [timeoutMs] elapses. Returns the elapsed milliseconds
     * on success (the measurable latency), null on timeout. [intervalMs] bounds polling cost.
     */
    inline fun pollUntilTrue(
        timeoutMs: Long,
        intervalMs: Long = 15,
        predicate: () -> Boolean,
    ): Long? {
        val start = SystemClock.uptimeMillis()
        while (SystemClock.uptimeMillis() - start <= timeoutMs) {
            if (predicate()) return SystemClock.uptimeMillis() - start
            Thread.sleep(intervalMs)
        }
        return null
    }

    /** Record one measurement for trend tracking (grep -e UX_METRIC). */
    fun metric(
        name: String,
        value: Number,
    ) {
        Log.i(TAG, "$name=$value")
    }
}

/** Minimal int rect used by [UxTestUtils.changedBoundingBox]. */
class Rect(
    val left: Int,
    val top: Int,
    val right: Int,
    val bottom: Int,
) {
    val width: Int
        get() = right - left + 1

    val height: Int
        get() = bottom - top + 1

    override fun toString(): String = "Rect(l=$left,t=$top,r=$right,b=$bottom ${width}x$height)"
}
