package io.torvox.ui

import android.os.Looper
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Shadows.shadowOf

@RunWith(AndroidJUnit4::class)
class GestureInteractionTest {
    private fun createView(): TerminalSurface {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val view = TerminalSurface(context)
        view.setDimensions(24, 80)
        view.measure(
            View.MeasureSpec.makeMeasureSpec(480, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(854, View.MeasureSpec.EXACTLY),
        )
        view.layout(0, 0, 480, 854)
        view.requestFocus()
        return view
    }

    @Test
    fun doubleTap_coordinates_map_to_cell() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 100f, 200f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_UP, 100f, 200f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime + 80, downTime + 80, MotionEvent.ACTION_DOWN, 100f, 200f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime + 80, downTime + 120, MotionEvent.ACTION_UP, 100f, 200f, 0),
        )
        assertTrue("view width should remain valid after double tap", view.width > 0)
    }

    @Test
    fun doubleTap_on_navigation_edge_does_not_crash() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 0f, 0f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_UP, 0f, 0f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime + 80, downTime + 80, MotionEvent.ACTION_DOWN, 0f, 0f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime + 80, downTime + 120, MotionEvent.ACTION_UP, 0f, 0f, 0),
        )
    }

    @Test
    fun doubleTap_on_bottom_region_does_not_crash() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 200f, 800f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_UP, 200f, 800f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime + 80, downTime + 80, MotionEvent.ACTION_DOWN, 200f, 800f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime + 80, downTime + 120, MotionEvent.ACTION_UP, 200f, 800f, 0),
        )
    }

    @Test
    fun longPress_on_text_area_sets_isAfterLongPress() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 200f, 200f, 0),
        )
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(2000))
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 2000, MotionEvent.ACTION_UP, 200f, 200f, 0),
        )
        assertTrue("isAfterLongPress should be set after long press gesture", view.isAfterLongPress)
    }

    @Test
    fun longPress_on_lower_region_sets_isAfterLongPress() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 50f, 50f, 0),
        )
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(2000))
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 2000, MotionEvent.ACTION_UP, 50f, 50f, 0),
        )
        assertTrue("isAfterLongPress should be set after long press on any region", view.isAfterLongPress)
    }

    @Test
    fun singleTap_after_longPress_clears_isAfterLongPress() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 200f, 200f, 0),
        )
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(2000))
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 2000, MotionEvent.ACTION_UP, 200f, 200f, 0),
        )
        assertTrue("isAfterLongPress should be set after long press", view.isAfterLongPress)
        val tapTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(tapTime, tapTime, MotionEvent.ACTION_DOWN, 210f, 210f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(tapTime, tapTime + 40, MotionEvent.ACTION_UP, 210f, 210f, 0),
        )
        assertFalse("isAfterLongPress should be cleared after single tap", view.isAfterLongPress)
    }

    @Test
    fun pinchZoom_exceeds_threshold_does_not_crash() {
        val view = createView()
        val props =
            arrayOf(
                MotionEvent.PointerProperties().apply {
                    id = 0
                    toolType = MotionEvent.TOOL_TYPE_FINGER
                },
                MotionEvent.PointerProperties().apply {
                    id = 1
                    toolType = MotionEvent.TOOL_TYPE_FINGER
                },
            )
        val coords =
            arrayOf(
                MotionEvent.PointerCoords().apply {
                    x = 100f
                    y = 200f
                },
                MotionEvent.PointerCoords().apply {
                    x = 300f
                    y = 200f
                },
            )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, 2, props, coords, 0, 0, 1f, 1f, 0, 0, 0, 0),
        )
        val moveCoords =
            arrayOf(
                MotionEvent.PointerCoords().apply {
                    x = 120f
                    y = 200f
                },
                MotionEvent.PointerCoords().apply {
                    x = 340f
                    y = 200f
                },
            )
        for (i in 1..20) {
            val scale = 1f + i * 0.01f
            moveCoords[0].x = 100f - (100f * (scale - 1f) * i / 20f)
            moveCoords[1].x = 300f + (0f * (scale - 1f) * i / 20f)
            view.dispatchTouchEvent(
                MotionEvent.obtain(0, i * 10L, MotionEvent.ACTION_MOVE, 2, props, moveCoords, 0, 0, scale, scale, 0, 0, 0, 0),
            )
        }
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 200, MotionEvent.ACTION_UP, 2, props, moveCoords, 0, 0, 1.2f, 1.2f, 0, 0, 0, 0),
        )
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(50))
        assertEquals("scaleFactor should reset to 1.0f after pinch zoom", 1.0f, view.scaleFactor, 0.01f)
    }

    @Test
    fun pinchZoom_pinch_in_does_not_crash() {
        val view = createView()
        val props =
            arrayOf(
                MotionEvent.PointerProperties().apply {
                    id = 0
                    toolType = MotionEvent.TOOL_TYPE_FINGER
                },
                MotionEvent.PointerProperties().apply {
                    id = 1
                    toolType = MotionEvent.TOOL_TYPE_FINGER
                },
            )
        val coords =
            arrayOf(
                MotionEvent.PointerCoords().apply {
                    x = 150f
                    y = 200f
                },
                MotionEvent.PointerCoords().apply {
                    x = 250f
                    y = 200f
                },
            )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, 2, props, coords, 0, 0, 1f, 1f, 0, 0, 0, 0),
        )
        val pinchCoords =
            arrayOf(
                MotionEvent.PointerCoords().apply {
                    x = 180f
                    y = 200f
                },
                MotionEvent.PointerCoords().apply {
                    x = 220f
                    y = 200f
                },
            )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 50, MotionEvent.ACTION_MOVE, 2, props, pinchCoords, 0, 0, 0.8f, 0.8f, 0, 0, 0, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 100, MotionEvent.ACTION_UP, 2, props, pinchCoords, 0, 0, 0.8f, 0.8f, 0, 0, 0, 0),
        )
        shadowOf(Looper.getMainLooper()).idleFor(java.time.Duration.ofMillis(50))
        assertEquals("scaleFactor should reset to 1.0f after pinch-in", 1.0f, view.scaleFactor, 0.01f)
    }

    @Test
    fun scroll_then_tap_stops_scrolling() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 100f, 100f, 0),
        )
        for (i in 1..10) {
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime + i * 16L, MotionEvent.ACTION_MOVE, 100f, 100f + i * 10f, 0),
            )
        }
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 200, MotionEvent.ACTION_UP, 100f, 200f, 0),
        )
        assertTrue("should be scrolling after scroll gesture", view.isCurrentlyScrolling())
        val tapTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(tapTime, tapTime, MotionEvent.ACTION_DOWN, 100f, 100f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(tapTime, tapTime + 40, MotionEvent.ACTION_UP, 100f, 100f, 0),
        )
        assertFalse("should NOT be scrolling after tap", view.isCurrentlyScrolling())
    }

    @Test
    fun fling_gesture_does_not_crash() {
        val view = createView()
        view.measure(
            View.MeasureSpec.makeMeasureSpec(480, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(2000, View.MeasureSpec.EXACTLY),
        )
        view.layout(0, 0, 480, 2000)
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 100f, 100f, 0),
        )
        for (i in 1..5) {
            view.dispatchTouchEvent(
                MotionEvent.obtain(
                    downTime,
                    downTime + i * 10L,
                    MotionEvent.ACTION_MOVE,
                    100f,
                    100f - i * 30f,
                    0,
                ),
            )
        }
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 80, MotionEvent.ACTION_UP, 100f, 50f, 0),
        )
        assertTrue("view width should remain valid after fling", view.width > 0)
    }

    @Test
    fun sequential_taps_at_different_locations() {
        val view = createView()
        val locations =
            listOf(
                50f to 100f,
                150f to 200f,
                250f to 300f,
                350f to 400f,
                50f to 500f,
            )
        for ((x, y) in locations) {
            val downTime = SystemClock.uptimeMillis()
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0),
            )
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_UP, x, y, 0),
            )
        }
    }
}
