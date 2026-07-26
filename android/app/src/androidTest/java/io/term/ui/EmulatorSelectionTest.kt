package io.term.ui

import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import androidx.test.core.app.ActivityScenario
import io.term.MainActivity
import org.junit.After
import org.junit.Before
import org.junit.Test

/**
 * Dispatches touch events directly to TerminalSurface via dispatchTouchEvent.
 * Host script captures adb screencaps. Logcat marks each stage.
 */
class EmulatorSelectionTest {
    private lateinit var scenario: ActivityScenario<MainActivity>
    private var surface: TerminalSurface? = null

    @Before
    fun setUp() {
        scenario = ActivityScenario.launch(MainActivity::class.java)
        Thread.sleep(10000)
        scenario.onActivity { activity ->
            val tv = findSurface(activity.window.decorView)
            surface = tv
            if (tv != null) {
                tv.requestFocus()
                tv.isFocusable = true
                tv.isFocusableInTouchMode = true
                android.util.Log.i("EmuTest", "surface=$tv size=${tv.width}x${tv.height}")
            } else {
                android.util.Log.e("EmuTest", "TerminalSurface not found!")
            }
        }
    }

    @After
    fun tearDown() {
        scenario.close()
    }

    @Test
    fun captureFromADB() {
        // Stage 1: Baseline
        android.util.Log.i("EmuStage", "STAGE_1_BASELINE")
        Thread.sleep(25_000)

        // Stage 2: Touch at terminal content area (Y=1100 ≈ middle of terminal)
        // GestureDetector-based long-press: keep DOWN for 2000ms then UP
        val startX = 200f
        val startY = 1150f
        val downTime = SystemClock.uptimeMillis()

        scenario.onActivity {
            surface?.dispatchTouchEvent(
                MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, startX, startY, 0).apply {
                    source = InputDevice.SOURCE_TOUCHSCREEN
                },
            )
        }
        android.util.Log.i("EmuTest", "Dispatched DOWN at ($startX, $startY)")

        // Wait for GestureDetector to fire onLongPress
        Thread.sleep(2000)

        android.util.Log.i("EmuTest", "Sending UP after 2000ms hold")
        val upTime = SystemClock.uptimeMillis()
        scenario.onActivity {
            surface?.dispatchTouchEvent(
                MotionEvent.obtain(downTime, upTime, MotionEvent.ACTION_UP, startX, startY, 0).apply {
                    source = InputDevice.SOURCE_TOUCHSCREEN
                },
            )
        }
        // Wait for selection UI to render
        Thread.sleep(2000)

        android.util.Log.i("EmuStage", "STAGE_2_TOUCH_DISPATCHED")
        Thread.sleep(25_000)

        android.util.Log.i("EmuStage", "ALL_STAGES_COMPLETE")
    }

    companion object {
        private fun findSurface(root: android.view.View): TerminalSurface? {
            if (root is TerminalSurface) return root
            if (root is android.view.ViewGroup) {
                for (i in 0 until root.childCount) {
                    findSurface(root.getChildAt(i))?.let { return it }
                }
            }
            return null
        }
    }
}
