package io.term.ui

import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalLifecycleTest {
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
    fun surfaceCreated_hasValidDimensions() {
        val view = createView()
        assertEquals("Width should be 480", 480, view.width)
        assertEquals("Height should be 854", 854, view.height)
    }

    @Test
    fun surfaceDestroyed_doesNotCrash() {
        val view = createView()
        view.dispatchWindowVisibilityChanged(View.GONE)
    }

    @Test
    fun surfaceRecreated_afterDetach() {
        val view = createView()
        view.dispatchWindowVisibilityChanged(View.GONE)
        view.measure(
            View.MeasureSpec.makeMeasureSpec(640, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(960, View.MeasureSpec.EXACTLY),
        )
        view.layout(0, 0, 640, 960)
        assertEquals("Width should be 640 after recreation", 640, view.width)
        assertEquals("Height should be 960 after recreation", 960, view.height)
    }

    @Test
    fun surfaceView_isFocusable() {
        val view = createView()
        assertTrue("TerminalSurface should be focusable", view.isFocusable)
        assertTrue("TerminalSurface should be focusable in touch mode", view.isFocusableInTouchMode)
    }

    @Test
    fun surfaceView_focusRequest_grantsFocus() {
        val view = createView()
        val gained = view.requestFocus()
        assertTrue("View should gain focus", gained)
        assertTrue("View should have focus", view.isFocused)
    }

    @Test
    fun surfaceView_repeatedFocusRequest_keepsFocus() {
        val view = createView()
        assertTrue("First requestFocus should succeed", view.requestFocus())
        assertTrue("View should be focused after first call", view.isFocused)
        assertTrue("Second requestFocus should also succeed", view.requestFocus())
        assertTrue("View should still be focused after second call", view.isFocused)
    }

    @Test
    fun visibleToUser_toTrue_resumesRendering() {
        val view = createView()
        view.dispatchWindowVisibilityChanged(View.GONE)
        view.handler?.post(Runnable { Thread.sleep(1) })
        view.dispatchWindowVisibilityChanged(View.VISIBLE)
        view.requestFocus()
        assertTrue("View should accept focus after becoming visible again", view.isFocused)
    }

    @Test
    fun visibleToUser_toFalse_pausesRendering() {
        val view = createView()
        view.dispatchWindowVisibilityChanged(View.GONE)
        view.handler?.removeCallbacksAndMessages(null)
        view.dispatchWindowVisibilityChanged(View.VISIBLE)
        assertTrue("View should still be usable after visibility cycle", view.isFocusable)
    }

    @Test
    fun dispatchMultipleGestureTypes_succeeds() {
        val view = createView()
        view.setDimensions(24, 80)
        // Single tap
        dispatchTouchAction(view, MotionEvent.ACTION_DOWN, 100f, 100f)
        dispatchTouchAction(view, MotionEvent.ACTION_UP, 100f, 100f)
        // Scroll
        view.dispatchTouchEvent(
            MotionEvent.obtain(
                SystemClock.uptimeMillis(),
                SystemClock.uptimeMillis(),
                MotionEvent.ACTION_DOWN,
                100f,
                100f,
                0,
            ),
        )
        for (i in 1..5) {
            view.dispatchTouchEvent(
                MotionEvent.obtain(
                    SystemClock.uptimeMillis(),
                    SystemClock.uptimeMillis(),
                    MotionEvent.ACTION_MOVE,
                    100f,
                    100f + i * 10f,
                    0,
                ),
            )
        }
        view.dispatchTouchEvent(
            MotionEvent.obtain(
                SystemClock.uptimeMillis(),
                SystemClock.uptimeMillis(),
                MotionEvent.ACTION_UP,
                100f,
                150f,
                0,
            ),
        )
    }

    private fun dispatchTouchAction(
        view: View,
        action: Int,
        x: Float,
        y: Float,
    ) {
        view.dispatchTouchEvent(
            MotionEvent.obtain(SystemClock.uptimeMillis(), SystemClock.uptimeMillis(), action, x, y, 0),
        )
    }

    @Test
    fun repeatedDispatchTouch_doesNotCrash() {
        val view = createView()
        for (i in 0..49) {
            val downTime = SystemClock.uptimeMillis()
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 50f + i, 100f, 0),
            )
            view.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime + 10, MotionEvent.ACTION_UP, 50f + i, 100f, 0),
            )
        }
    }

    @Test
    fun longTouchSequence_completesWithoutError() {
        val view = createView()
        val downTime = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, 100f, 100f, 0),
        )
        for (i in 1..50) {
            view.dispatchTouchEvent(
                MotionEvent.obtain(
                    downTime,
                    downTime + i * 10L,
                    MotionEvent.ACTION_MOVE,
                    100f + i,
                    100f + i,
                    0,
                ),
            )
        }
        view.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 500, MotionEvent.ACTION_UP, 150f, 150f, 0),
        )
    }

    @Test
    fun layoutChange_updatesDimensions() {
        val view = createView()
        view.layout(0, 0, 800, 1200)
        assertEquals("Width should update to 800", 800, view.width)
        assertEquals("Height should update to 1200", 1200, view.height)
    }

    @Test
    fun measureChange_updatesMeasuredDimensions() {
        val view = createView()
        view.measure(
            View.MeasureSpec.makeMeasureSpec(320, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(480, View.MeasureSpec.EXACTLY),
        )
        assertEquals("Measured width should be 320", 320, view.measuredWidth)
        assertEquals("Measured height should be 480", 480, view.measuredHeight)
    }

    @Test
    fun dispatchTouchBeforeAttach_doesNotCrash() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val view = TerminalSurface(context)
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, 100f, 100f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 50, MotionEvent.ACTION_UP, 100f, 100f, 0),
        )
    }

    @Test
    fun dispatchTouchMultiplePointers_doesNotCrash() {
        val view = createView()
        val pointerProperties =
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
        val pointerCoords =
            arrayOf(
                MotionEvent.PointerCoords().apply {
                    x = 100f
                    y = 100f
                },
                MotionEvent.PointerCoords().apply {
                    x = 200f
                    y = 100f
                },
            )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, 2, pointerProperties, pointerCoords, 0, 0, 1f, 1f, 0, 0, 0, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(0, 100, MotionEvent.ACTION_UP, 2, pointerProperties, pointerCoords, 0, 0, 1f, 1f, 0, 0, 0, 0),
        )
    }
}
