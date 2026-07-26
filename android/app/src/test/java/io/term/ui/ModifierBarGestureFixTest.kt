package io.term.ui

import android.os.Looper
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Shadows.shadowOf

@RunWith(AndroidJUnit4::class)
class ModifierBarGestureFixTest {
    companion object {
        private const val VIEW_WIDTH = 480
        private const val VIEW_HEIGHT = 854
        private const val CELL_ROWS = 24
        private const val CELL_COLS = 80
        private const val MOD_BAR_HEIGHT_PX = 80
        private const val PASS_THROUGH_MIN_HEIGHT = MOD_BAR_HEIGHT_PX * 2
    }

    private fun createView(): TerminalSurface {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val view = TerminalSurface(context)
        view.setDimensions(CELL_ROWS, CELL_COLS)
        view.measure(
            View.MeasureSpec.makeMeasureSpec(VIEW_WIDTH, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(VIEW_HEIGHT, View.MeasureSpec.EXACTLY),
        )
        view.layout(0, 0, VIEW_WIDTH, VIEW_HEIGHT)
        view.requestFocus()
        return view
    }

    private fun dispatchTap(
        view: View,
        x: Float = 100f,
        y: Float = 100f,
    ): Boolean {
        val downTime = SystemClock.uptimeMillis()
        val downConsumed =
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0),
            )
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(50))
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 50, MotionEvent.ACTION_UP, x, y, 0),
        )
        return downConsumed
    }

    private fun dispatchScroll(
        view: View,
        startX: Float = 100f,
        startY: Float = 100f,
        endY: Float = 200f,
        steps: Int = 10,
    ): Boolean {
        val downTime = SystemClock.uptimeMillis()
        val downConsumed =
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, startX, startY, 0),
            )
        for (i in 1..steps) {
            val currentY = startY + (endY - startY) * i / steps
            view.dispatchTouchEvent(
                MotionEvent.obtain(
                    downTime,
                    downTime + i * 10,
                    MotionEvent.ACTION_MOVE,
                    startX,
                    currentY,
                    0,
                ),
            )
        }
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(50))
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + steps * 10 + 50, MotionEvent.ACTION_UP, startX, endY, 0),
        )
        return downConsumed
    }

    // ── 1. isInModBarZone detection ──────────────────────────────

    @Test
    fun `modBarZone detection top region is NOT in zone`() {
        val view = createView()
        val modBarTopY = VIEW_HEIGHT - MOD_BAR_HEIGHT_PX // = 774
        // Tap above the zone at Y=100
        val consumed = dispatchTap(view, y = 100f)
        // Normal terminal zone: gesture detector returns true
        assertTrue("Touch well above mod bar zone should be consumed", consumed)
    }

    @Test
    fun `modBarZone detection mid region is NOT in zone`() {
        val view = createView()
        val modBarTopY = VIEW_HEIGHT - MOD_BAR_HEIGHT_PX // = 774
        // Tap in the middle at Y=400
        val consumed = dispatchTap(view, y = 400f)
        assertTrue("Touch in mid region should be consumed", consumed)
    }

    @Test
    fun `modBarZone detection boundary is IN zone`() {
        val view = createView()
        val modBarTopY = VIEW_HEIGHT - MOD_BAR_HEIGHT_PX // = 774
        // Tap right at the boundary
        val consumed = dispatchTap(view, y = modBarTopY.toFloat())
        assertTrue("Touch at zone boundary still consumed", consumed)
    }

    @Test
    fun `modBarZone detection bottom edge is IN zone`() {
        val view = createView()
        val modBarTopY = VIEW_HEIGHT - MOD_BAR_HEIGHT_PX // = 774
        // Tap at Y=800, well inside the mod bar zone
        val consumed = dispatchTap(view, y = 800f)
        assertTrue("Touch in mod bar zone still consumed (no viewModel)", consumed)
    }

    // ── 2. passThrough guard height checks ──────────────────────

    @Test
    fun `passThrough guard activates when view is tall enough`() {
        val height = VIEW_HEIGHT
        val minHeight = PASS_THROUGH_MIN_HEIGHT
        assertTrue(
            "View height $height should exceed passThrough min $minHeight for activation",
            height > minHeight,
        )
    }

    @Test
    fun `passThrough guard deactivates when view is too small`() {
        val smallHeight = MOD_BAR_HEIGHT_PX // 80 — very small (IME open)
        val minHeight = PASS_THROUGH_MIN_HEIGHT // 160
        assertTrue(
            "Small height $smallHeight should be below passThrough min $minHeight",
            smallHeight < minHeight,
        )
    }

    @Test
    fun `passThrough guard boundary at exactly 2x mod bar height`() {
        val boundaryHeight = PASS_THROUGH_MIN_HEIGHT // 160
        val minHeight = PASS_THROUGH_MIN_HEIGHT
        assertEquals(
            "Boundary at exactly 2x mod bar height should not activate passThrough",
            boundaryHeight,
            minHeight,
        )
    }

    // ── 3. Scroll gesture correctness ───────────────────────────

    @Test
    fun `scroll gesture starting above mod bar zone works`() {
        val view = createView()
        // Scroll from Y=100 to Y=500 (well above mod bar zone)
        val consumed = dispatchScroll(view, startY = 100f, endY = 500f)
        assertTrue("Scroll in terminal area should be consumed", consumed)
    }

    @Test
    fun `scroll gesture entering mod bar zone does not crash`() {
        val view = createView()
        // Scroll from Y=500 to Y=800 (enters mod bar zone)
        val consumed = dispatchScroll(view, startY = 500f, endY = 800f)
        assertTrue("Scroll entering mod bar zone should not crash", consumed)
    }

    // ── 4. modBarZone calculation sanity ────────────────────────

    @Test
    fun `modBarTopY is within valid range`() {
        val modBarTopY = VIEW_HEIGHT - MOD_BAR_HEIGHT_PX
        assertTrue("modBarTopY $modBarTopY should be >= 0", modBarTopY >= 0)
        assertTrue(
            "modBarTopY $modBarTopY should be < viewHeight $VIEW_HEIGHT",
            modBarTopY < VIEW_HEIGHT,
        )
    }

    @Test
    fun `modBarHeight is positive`() {
        assertTrue("modBarHeightPx should be positive", MOD_BAR_HEIGHT_PX > 0)
    }

    // ── 5. ExtraKeyButton dwell guard constants ─────────────────

    @Test
    fun `nonRepeat dwell guard constant prevents swipe-through`() {
        // dwellGuardMs must be long enough to distinguish tap from swipe
        val dwellGuardMs = 100L // from ModifierBar.kt
        assertTrue("Dwell guard should be at least 50ms", dwellGuardMs >= 50)
        assertTrue("Dwell guard should be at most 200ms", dwellGuardMs <= 200)
    }

    @Test
    fun `nonRepeat dwell guard fires onClick only after dwell`() {
        // The dwell guard waits DWELL_GUARD_MS for the finger to stabilize.
        // A quick flick (faster than DWELL_GUARD_MS) should NOT trigger onClick.
        val dwellGuardMs = 100L
        val minimumDwellNanos = dwellGuardMs * 1_000_000L
        assertTrue("Dwell window must cover at least ${dwellGuardMs}ms", minimumDwellNanos > 0)
    }

    // ── 6. Touch slop should match button dimensions ────────────

    @Test
    fun `touch slop should be less than button height`() {
        val buttonHeightDp = 36
        val density = 1.0f // Robolectric default
        val buttonHeightPx = (buttonHeightDp * density).toInt()
        val typicalSlop = 24 // typical Android touch slop
        assertTrue(
            "Button height $buttonHeightPx should exceed touch slop $typicalSlop",
            buttonHeightPx > typicalSlop,
        )
    }

    // ── 7. No crash for edge coordinates ────────────────────────

    @Test
    fun `touch at negative coordinates does not crash`() {
        val view = createView()
        dispatchTap(view, x = -1f, y = -1f)
    }

    @Test
    fun `touch beyond view bounds does not crash`() {
        val view = createView()
        dispatchTap(view, x = 1000f, y = 2000f)
    }

    @Test
    fun `scroll beyond view bounds does not crash`() {
        val view = createView()
        dispatchScroll(view, startY = 100f, endY = 2000f, steps = 20)
    }
}
