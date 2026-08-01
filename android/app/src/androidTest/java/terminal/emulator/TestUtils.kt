package terminal.emulator

import android.app.Activity
import android.graphics.Bitmap
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.platform.app.InstrumentationRegistry
import terminal.emulator.bridge.Bridge
import java.io.File

// ── Data model ──────────────────────────────────────

fun AndroidComposeTestRule<*, *>.waitForSession(timeoutMs: Long = 60_000) {
    System.setProperty("test.minSurface", "true")
    // MainActivity.onCreate() requests POST_NOTIFICATIONS on first run
    // (Android 13+); the permission dialog overlays the activity and
    // blocks waitUntil(TerminalScreen). Grant up front — idempotent,
    // and a pending request dialog is dismissed once the permission is
    // granted underneath it.
    grantNotificationPermission()
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

fun grantNotificationPermission() {
    val instrumentation = androidx.test.platform.app.InstrumentationRegistry.getInstrumentation()
    instrumentation.uiAutomation.grantRuntimePermission(
        instrumentation.targetContext.packageName,
        android.Manifest.permission.POST_NOTIFICATIONS,
    )
}

fun AndroidComposeTestRule<*, *>.getBridge(): Bridge? {
    var bridge: Bridge? = null
    val rule = activityRule as ActivityScenarioRule<*>
    val deadlineMs = System.currentTimeMillis() + 15_000
    while (bridge == null && System.currentTimeMillis() < deadlineMs) {
        Thread.sleep(100)
        rule.scenario.onActivity { activity: android.app.Activity ->
            bridge = (activity as MainActivity).runtime.bridge()
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

fun injectLongPress(
    view: View,
    x: Float,
    y: Float,
) {
    val dt = SystemClock.uptimeMillis()
    // Must NOT block the main thread — GestureDetector uses a Handler on the
    // main-thread looper.  If we dispatch DOWN then sleep(800) on the main
    // thread the long-press timer message is queued but never processed before
    // ACTION_UP arrives — GestureDetector then cancels the pending long-press
    // and treats the gesture as a tap (the root cause of "Action_Dismiss not
    // displayed" in text-selection tests).
    view.post {
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0))
    }
    // Sleep on the CALLER test thread — the main thread is free to process
    // the GestureDetector long-press handler (~500 ms) and the selection
    // state update before MOVE / UP arrive.
    try {
        Thread.sleep(1200)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    view.post {
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt + 1200, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0))
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt + 1250, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0))
    }
    // Give the main thread time to process MOVE/UP before the caller
    // continues to the "Then" assertion step.
    try {
        Thread.sleep(300)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
}

fun injectTap(
    view: View,
    x: Float,
    y: Float,
) {
    val dt = SystemClock.uptimeMillis()
    view.post {
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt, MotionEvent.ACTION_DOWN, x, y, 0))
    }
    try {
        Thread.sleep(100)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    view.post {
        view.dispatchTouchEvent(MotionEvent.obtain(dt, dt + 100, MotionEvent.ACTION_UP, x, y, 0))
    }
}

fun injectDoubleTap(
    view: View,
    x: Float,
    y: Float,
) {
    injectTap(view, x, y)
    try {
        Thread.sleep(200)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    injectTap(view, x, y)
}

fun injectTripleTap(
    view: View,
    x: Float,
    y: Float,
) {
    injectTap(view, x, y)
    try {
        Thread.sleep(200)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    injectTap(view, x, y)
    try {
        Thread.sleep(200)
    } catch (_: InterruptedException) {
        Thread.currentThread().interrupt()
    }
    injectTap(view, x, y)
}

// ── Selection assertion helpers ─────────────────────

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
