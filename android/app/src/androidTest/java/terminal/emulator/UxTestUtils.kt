package terminal.emulator

import android.graphics.Bitmap
import android.os.SystemClock
import android.util.Log
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import androidx.test.uiautomator.UiDevice
import java.io.File
import terminal.emulator.bridge.Bridge

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
    fun changedPixelCount(before: Bitmap, after: Bitmap, tolerance: Int = 12): Int {
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
     * 两截图在纵向区间 [top, bottom) 内按 [step] 步长抽样的差异像素数：
     * 三通道差和超 [threshold] 记为差异（步长只影响统计速度，不放宽判定）。
     * 稀疏条带比对与 SGR 斜体裁剪区同款口径，故收归此处一处实现。
     */
    fun countDiffInBand(
        first: Bitmap,
        second: Bitmap,
        top: Int,
        bottom: Int,
        step: Int,
        threshold: Int,
    ): Int {
        var count = 0
        for (y in top until bottom step step) {
            for (x in 0 until first.width step step) {
                val delta =
                    kotlin.math.abs(
                        android.graphics.Color.red(first.getPixel(x, y)) -
                            android.graphics.Color.red(second.getPixel(x, y)),
                    ) +
                        kotlin.math.abs(
                            android.graphics.Color.green(first.getPixel(x, y)) -
                                android.graphics.Color.green(second.getPixel(x, y)),
                        ) +
                        kotlin.math.abs(
                            android.graphics.Color.blue(first.getPixel(x, y)) -
                                android.graphics.Color.blue(second.getPixel(x, y)),
                        )
                if (delta > threshold) count++
            }
        }
        return count
    }

    /**
     * Bounding box of pixels differing between [before] and [after], or null when nothing changed
     * beyond [tolerance]. Used by the cursor geometry test: diffing cursor-at-col-A vs
     * cursor-at-col-B isolates exactly the two cursor quads — their bbox IS the measurable cursor
     * geometry, independent of theme colors.
     */
    fun changedBoundingBox(before: Bitmap, after: Bitmap, tolerance: Int = 12): Rect? {
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
    inline fun pollUntilTrue(timeoutMs: Long, intervalMs: Long = 15, predicate: () -> Boolean): Long? {
        val start = SystemClock.uptimeMillis()
        while (SystemClock.uptimeMillis() - start <= timeoutMs) {
            if (predicate()) return SystemClock.uptimeMillis() - start
            Thread.sleep(intervalMs)
        }
        return null
    }

    /** Record one measurement for trend tracking (grep -e UX_METRIC). */
    fun metric(name: String, value: Number) {
        Log.i(TAG, "$name=$value")
    }
}

// ── 像素判读 ─────────────────────────────────────────

/**
 * 色相判定：某通道显著高于其余两通道即计入该色相计数。
 *
 * 阈值同时挡住抗锯齿边缘与主题自身的强调色；三个色相共用同一判据，
 * 免得「红按红判、绿按绿判」各自漂移成不可比的数。
 */
private fun countHuePixels(shot: Bitmap, hue: (red: Int, green: Int, blue: Int) -> Boolean): Int {
    var count = 0
    // 步长 2 采样：步长 3 在阈值边缘会漏掉真实字形，背景纯色区加密后仍为 0。
    for (y in 0 until shot.height step 2) {
        for (x in 0 until shot.width step 2) {
            val pixel = shot.getPixel(x, y)
            if (hue(android.graphics.Color.red(pixel), android.graphics.Color.green(pixel), android.graphics.Color.blue(pixel))) count++
        }
    }
    return count
}

fun countReddishPixels(shot: Bitmap): Int = countHuePixels(shot) { red, green, blue ->
    red > 110 && red - blue > 50 && red - green > 30
}

fun countGreenishPixels(shot: Bitmap): Int = countHuePixels(shot) { red, green, blue ->
    green > 110 && green - red > 50 && green - blue > 30
}

fun countBluishPixels(shot: Bitmap): Int = countHuePixels(shot) { red, green, blue ->
    blue > 110 && blue - red > 50 && blue - green > 30
}

/** 单像素三通道差和（阈值由调用方判定）。 */
fun pixelChannelDelta(first: Int, second: Int): Int =
    kotlin.math.abs(android.graphics.Color.red(first) - android.graphics.Color.red(second)) +
        kotlin.math.abs(android.graphics.Color.green(first) - android.graphics.Color.green(second)) +
        kotlin.math.abs(android.graphics.Color.blue(first) - android.graphics.Color.blue(second))

/** 屏幕像素的感知亮度（0..255）；坐标越界返回 -1。 */
fun pixelLuminance(shot: Bitmap, x: Int, y: Int): Int {
    if (x < 0 || x >= shot.width || y < 0 || y >= shot.height) return -1
    val pixel = shot.getPixel(x, y)
    return (
        (pixel shr 16 and 0xFF) * 299 +
            (pixel shr 8 and 0xFF) * 587 +
            (pixel and 0xFF) * 114
        ) / 1000
}

// ── 网格定位写入 ─────────────────────────────────────

/** SGR 转义序列：`ESC [ … m`。不占列，故落格判据须先剥除。 */
private val SGR_SEQUENCE = Regex("\u001B\\[[0-9;]*m")

/**
 * 视口 [row] 行首列起写入 [text] 并轮询至**渲染光标**落在该行文本末列。
 *
 * 落格判据取渲染光标而非文本包含：光标位置由 VT 产出、与单元格内容同源，
 * 文本包含则在标记被换行截断时同样成立。写入前只把光标归位、**不清屏**：
 * 清屏会抹掉同一次调用序列里先前写下的标记（实测三色只剩最后一个，红/绿像素
 * 恒为 0、蓝独活）；光标归位已足够让每轮轮询从可区分的状态出发。
 * SGR 不占列，期望列按剥除转义后的可见长度计。
 */
fun placeTextAtRow(bridge: Bridge, row: Int, text: String, timeoutMs: Long = 15_000) {
    val expectedColumn = SGR_SEQUENCE.replace(text, "").length
    val homed = bridge.feedTerminal("\u001B[1;1H".toByteArray(Charsets.UTF_8))
    assertTrue("光标归位送显失败", homed)
    val fed = bridge.feedTerminal("\u001B[${row + 1};1H$text".toByteArray(Charsets.UTF_8))
    assertTrue("标记送显失败", fed)
    val landed = UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 50) {
        val packed = bridge.cursorViewportPacked()
        packed >= 0 &&
            (packed shr 32).toInt() == row &&
            (packed and 0xffffffffL).toInt() == expectedColumn
    }
    // 断言信息不得含裸控制字符：ESC 会让 JUnit XML 报告无法解析，失败详情随之整体丢失。
    assertNotNull(
        "标记必须落格于 ($row,$expectedColumn): [${SGR_SEQUENCE.replace(text, "")}]",
        landed,
    )
}

/** Minimal int rect used by [UxTestUtils.changedBoundingBox]. */
class Rect(val left: Int, val top: Int, val right: Int, val bottom: Int) {
    val width: Int
        get() = right - left + 1

    val height: Int
        get() = bottom - top + 1

    override fun toString(): String = "Rect(l=$left,t=$top,r=$right,b=$bottom ${width}x$height)"
}
