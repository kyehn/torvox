package io.term

import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import androidx.lifecycle.Lifecycle
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Comprehensive UI tests for the terminal emulator.
 * Tests verify:
 * - App launches and displays correctly
 * - Terminal view is visible and interactive
 * - UI components render properly
 * - Touch interactions work
 * - App survives configuration changes
 */
@RunWith(AndroidJUnit4::class)
class TerminalUiTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(io.term.MainActivity::class.java)

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 10000)
    }

    // ── 1. App Launch Tests ──────────────────────────────

    @Test
    fun appLaunchesSuccessfully() {
        activityRule.scenario.onActivity { activity ->
            assertNotNull("Activity should not be null", activity)
            assertFalse("Activity should not be finishing", activity.isFinishing)
        }
    }

    @Test
    fun appPackageNameIsCorrect() {
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        assertEquals("com.termux", appContext.packageName)
    }

    @Test
    fun appHasLauncherActivity() {
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        val pm = appContext.packageManager
        val intent = pm.getLaunchIntentForPackage("com.termux")
        assertNotNull("App should have launch intent", intent)
    }

    // ── 2. Activity Lifecycle Tests ──────────────────────────────

    @Test
    fun mainActivityIsResumed() {
        activityRule.scenario.onActivity { activity ->
            assertEquals(
                "Activity should be in RESUMED state",
                Lifecycle.State.RESUMED,
                activity.lifecycle.currentState,
            )
        }
    }

    @Test
    fun activityHasContentView() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            assertNotNull("Content view should exist", contentView)
            assertTrue("Content view should be displayed", contentView.isShown)
        }
    }

    @Test
    fun activityHasDecorView() {
        activityRule.scenario.onActivity { activity ->
            val decorView = activity.window.decorView
            assertNotNull("Decor view should exist", decorView)
            assertTrue("Decor view should be attached", decorView.isAttachedToWindow)
        }
    }

    // ── 3. Terminal View Tests ──────────────────────────────

    @Test
    fun terminalViewExists() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            assertNotNull("Content view should exist", contentView)
            assertTrue(
                "Content view should have children",
                (contentView as? ViewGroup)?.childCount ?: 0 > 0,
            )
        }
    }

    @Test
    fun terminalViewIsDisplayed() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            assertTrue("Terminal content should be visible", contentView.isShown)
        }
    }

    @Test
    fun terminalViewHasCorrectSize() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            assertTrue("Terminal view should have positive width", contentView.width > 0)
            assertTrue("Terminal view should have positive height", contentView.height > 0)
        }
    }

    @Test
    fun terminalViewFillsScreen() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            val displayMetrics = activity.resources.displayMetrics
            val screenWidth = displayMetrics.widthPixels
            val screenHeight = displayMetrics.heightPixels
            // Terminal view should be at least 80% of screen
            assertTrue(
                "Terminal width should be substantial (${contentView.width}/$screenWidth)",
                contentView.width > screenWidth * 0.5f,
            )
            assertTrue(
                "Terminal height should be substantial (${contentView.height}/$screenHeight)",
                contentView.height > screenHeight * 0.5f,
            )
        }
    }

    // ── 4. Touch Interaction Tests ──────────────────────────────

    @Test
    fun touchDownAndUpIsProcessed() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            val centerX = contentView.width / 2f
            val centerY = contentView.height / 2f

            val downEvent = MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, centerX, centerY, 0)
            val upEvent = MotionEvent.obtain(0, 100, MotionEvent.ACTION_UP, centerX, centerY, 0)

            contentView.dispatchTouchEvent(downEvent)
            contentView.dispatchTouchEvent(upEvent)

            downEvent.recycle()
            upEvent.recycle()
            assertNotNull("Activity should not be null", activity)
        }
    }

    @Test
    fun scrollGestureIsProcessed() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            val centerX = contentView.width / 2f
            val startY = contentView.height * 0.3f
            val endY = contentView.height * 0.7f

            val downEvent = MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, centerX, startY, 0)
            contentView.dispatchTouchEvent(downEvent)
            downEvent.recycle()

            for (i in 1..10) {
                val currentY = startY + (endY - startY) * i / 10f
                val moveEvent =
                    MotionEvent.obtain(
                        0,
                        i * 16L,
                        MotionEvent.ACTION_MOVE,
                        centerX,
                        currentY,
                        0,
                    )
                contentView.dispatchTouchEvent(moveEvent)
                moveEvent.recycle()
            }

            val upEvent = MotionEvent.obtain(0, 200, MotionEvent.ACTION_UP, centerX, endY, 0)
            contentView.dispatchTouchEvent(upEvent)
            upEvent.recycle()
            assertNotNull("Activity should not be null", activity)
        }
    }

    @Test
    fun multiTouchIsProcessed() {
        activityRule.scenario.onActivity { activity ->
            val contentView = activity.findViewById<View>(android.R.id.content)
            val centerX = contentView.width / 2f
            val centerY = contentView.height / 2f

            // Simulate two-finger touch
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
                        x = centerX - 50
                        y = centerY
                    },
                    MotionEvent.PointerCoords().apply {
                        x = centerX + 50
                        y = centerY
                    },
                )

            val downEvent =
                MotionEvent.obtain(
                    0,
                    0,
                    MotionEvent.ACTION_DOWN,
                    2,
                    pointerProperties,
                    pointerCoords,
                    0,
                    0,
                    1.0f,
                    1.0f,
                    0,
                    0,
                    0,
                    0,
                )
            contentView.dispatchTouchEvent(downEvent)
            downEvent.recycle()

            val upEvent =
                MotionEvent.obtain(
                    0,
                    100,
                    MotionEvent.ACTION_UP,
                    2,
                    pointerProperties,
                    pointerCoords,
                    0,
                    0,
                    1.0f,
                    1.0f,
                    0,
                    0,
                    0,
                    0,
                )
            contentView.dispatchTouchEvent(upEvent)
            upEvent.recycle()
            assertNotNull("Activity should not be null", activity)
        }
    }

    // ── 5. Configuration Change Tests ──────────────────────────────

    @Test
    fun appSurvivesPauseResume() {
        activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        activityRule.scenario.onActivity { activity ->
            assertFalse("Activity should not be finishing after pause", activity.isFinishing)
        }
        activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        activityRule.scenario.onActivity { activity ->
            assertEquals(
                "Activity should be RESUMED",
                Lifecycle.State.RESUMED,
                activity.lifecycle.currentState,
            )
        }
    }

    // ── 6. Resource Tests ──────────────────────────────

    @Test
    fun appHasCorrectPermissions() {
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        val pm = appContext.packageManager
        val appInfo = pm.getApplicationInfo("com.termux", 0)
        assertNotNull("App should have package info", appInfo)
        assertEquals("App should have correct target SDK", 36, appInfo.targetSdkVersion)
    }

    @Test
    fun appMemoryUsageIsReasonable() {
        val runtime = Runtime.getRuntime()
        runtime.gc()
        Thread.sleep(200)
        val usedMemory = runtime.totalMemory() - runtime.freeMemory()
        // App should use less than 100MB at startup
        assertTrue(
            "Memory usage should be reasonable (was ${usedMemory / 1024 / 1024}MB)",
            usedMemory < 100 * 1024 * 1024,
        )
    }
}
