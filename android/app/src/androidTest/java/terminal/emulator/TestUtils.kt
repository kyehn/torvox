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
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import terminal.emulator.bridge.Bridge

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
    // Dismiss permission dialog if it blocks the drawer button (CI cold start on PlayStore image).
    runCatching {
        val d = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        if (d.hasObject(By.text("Allow")) || d.hasObject(By.text("ALLOW"))) {
            d.findObject(By.text("Allow"))?.click() ?: d.findObject(By.text("ALLOW"))?.click()
            Thread.sleep(500)
        }
    }
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    // Try multiple selectors for the drawer button: desc, res (testTagsAsResourceId), and Compose
    // tag.
    val clicked =
        runCatching {
            if (device.wait(Until.hasObject(By.desc("Open session drawer")), 5000)) {
                device.findObject(By.desc("Open session drawer"))?.click()
                true
            } else if (device.wait(Until.hasObject(By.res("Key_DRAWER")), 3000)) {
                device.findObject(By.res("Key_DRAWER"))?.click()
                true
            } else {
                onNodeWithTag("Key_DRAWER").performClick()
                true
            }
        }
            .getOrDefault(false)
    if (!clicked) {
        runCatching { onNodeWithTag("Key_DRAWER").performClick() }
        runCatching {
            if (device.wait(Until.hasObject(By.res("Key_DRAWER")), 3000)) {
                device.findObject(By.res("Key_DRAWER"))?.click()
            }
        }
    }
    waitForIdle()
    // Ensure drawer content is composed before caller proceeds. Check both Compose tag and UiDevice
    // texts.
    val drawerVisible =
        runCatching {
            waitUntil(timeoutMillis = 10000) {
                val composeVisible =
                    runCatching {
                        onNodeWithTag("SessionDrawer", useUnmergedTree = true).assertIsDisplayed()
                        true
                    }
                        .getOrDefault(false)
                if (composeVisible) return@waitUntil true
                device.hasObject(By.text("Sessions")) ||
                    device.hasObject(By.res("SessionDrawer")) ||
                    device.hasObject(By.res("SettingsButton"))
            }
        }
            .isSuccess
    if (!drawerVisible) {
        // Fallback: UiDevice wait for drawer header
        runCatching {
            device.wait(Until.hasObject(By.text("Sessions")), 3000) ||
                device.wait(Until.hasObject(By.res("SessionDrawer")), 3000)
        }
        Thread.sleep(500)
    }
}

fun AndroidComposeTestRule<*, *>.openSettings() {
    // Grant notification permission before probing: the permission dialog overlays
    // the activity and blocks Settings navigation on first run (CI cold start on PlayStore image).
    runCatching { grantNotificationPermission() }
    // Also dismiss the system permission dialog if it is still visible (PlayStore image shows
    // Allow/Deny).
    runCatching {
        val d = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        if (d.hasObject(By.text("Allow"))) d.findObject(By.text("Allow"))?.click()
        if (d.hasObject(By.text("ALLOW"))) d.findObject(By.text("ALLOW"))?.click()
        if (d.hasObject(By.text("Deny"))) {
            // Do not click Deny; just grant via uiAutomation and wait
            Thread.sleep(300)
        }
    }
    // Fast-path: Settings may already be visible (shared activity across cucumber scenarios).
    val settingsAlreadyVisible =
        runCatching {
            onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed()
        }
            .isSuccess
    if (settingsAlreadyVisible) return
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    if (
        device.hasObject(By.text("Font Family")) ||
        device.hasObject(By.res("SettingsScreen")) ||
        device.hasObject(By.text("Appearance"))
    ) {
        return
    }
    // Try direct Compose click on SettingsButton even when drawer is closed — the drawer content is
    // composed off-screen (ModalNavigationDrawer) and the button is still in the semantics tree.
    val directClickOpened =
        runCatching {
            onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
            waitForIdle()
            waitForSettingsScreenProbe(8000)
        }
            .getOrDefault(false)
    if (directClickOpened) return
    // Try UiAutomator on the button's testTag resource id (testTagsAsResourceId = true) and
    // desc/text.
    val uiResClickOpened =
        runCatching {
            val found =
                device.wait(Until.hasObject(By.res("SettingsButton")), 3000) ||
                    device.wait(Until.hasObject(By.desc("Settings")), 3000) ||
                    device.wait(Until.hasObject(By.text("Settings")), 3000)
            if (found) {
                val obj =
                    device.findObject(By.res("SettingsButton"))
                        ?: device.findObject(By.desc("Settings"))
                        ?: device.findObject(By.text("Settings"))
                obj?.click()
                waitForIdle()
                waitForSettingsScreenProbe(8000)
            } else {
                false
            }
        }
            .getOrDefault(false)
    if (uiResClickOpened) return
    // Fallback: open drawer explicitly then click Settings. Retry up to 3 times for CI flakiness.
    repeat(3) { attempt ->
        openDrawer()
        // After drawer is open, try Compose first (most reliable), then UiDevice res/desc/text.
        val composeAfterDrawer =
            runCatching {
                onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
                waitForIdle()
                waitForSettingsScreenProbe(8000)
            }
                .getOrDefault(false)
        if (composeAfterDrawer) return
        val selectors =
            listOf(
                By.res("SettingsButton"),
                By.desc("Settings"),
                By.text("Settings"),
                By.res("SearchButton"),
            )
        var selectorClicked = false
        for (selector in selectors) {
            // Skip SearchButton — we only use it to confirm drawer is open, not to click Settings
            if (selector == By.res("SearchButton")) {
                if (!device.hasObject(selector)) continue else break
            }
            selectorClicked =
                runCatching {
                    if (device.wait(Until.hasObject(selector), 5000)) {
                        device.findObject(selector)?.click()
                        true
                    } else {
                        false
                    }
                }
                    .getOrDefault(false)
            if (selectorClicked) break
        }
        if (selectorClicked) {
            waitForIdle()
            if (waitForSettingsScreenProbe(8000)) return
            // Also try a second Compose click after UiDevice click (covers scrim race)
            runCatching {
                onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
                waitForIdle()
                if (waitForSettingsScreenProbe(5000)) return
            }
        }
        if (attempt < 2) Thread.sleep(700)
    }
    waitForSettingsScreen(timeoutMs = 60_000)
}

private fun AndroidComposeTestRule<*, *>.waitForSettingsScreenProbe(timeoutMs: Long): Boolean {
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    val deadline = System.currentTimeMillis() + timeoutMs
    while (System.currentTimeMillis() < deadline) {
        val visible =
            runCatching {
                onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed()
                true
            }
                .getOrDefault(false)
        if (visible) return true
        // Check both text and resource id (testTagsAsResourceId) for robustness on PlayStore image
        if (
            device.hasObject(By.text("Font Family")) ||
            device.hasObject(By.res("SettingsScreen")) ||
            device.hasObject(By.text("Appearance")) ||
            device.hasObject(By.text("Font Size")) ||
            device.hasObject(By.text("Cursor")) ||
            device.hasObject(
                By.text("Sessions"),
            ) // drawer still open but Settings should be overlay
        ) {
            // If we see Sessions but not Settings, drawer is open but Settings not yet — keep waiting
            if (
                device.hasObject(By.text("Font Family")) ||
                device.hasObject(By.res("SettingsScreen")) ||
                device.hasObject(By.text("Appearance"))
            ) {
                return true
            }
        }
        Thread.sleep(200)
    }
    return false
}

/** Wait until [SettingsScreen] test-tag is displayed. */
fun AndroidComposeTestRule<*, *>.waitForSettingsScreen(timeoutMs: Long = 60_000) {
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    // CI PlayStore emulator (swiftshader, 2 cores, 1536M) is much slower than local google_apis:
    // 30s still timed out in 32833141258 (8 failures). Use 60s and poll both Compose tag and
    // UiDevice resource/text so we succeed even when Compose idle is delayed or semantics are merged.
    waitUntil(timeoutMillis = timeoutMs) {
        try {
            onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertIsDisplayed()
            true
        } catch (_: AssertionError) {
            device.hasObject(By.res("SettingsScreen")) ||
                device.hasObject(By.text("Font Family")) ||
                device.hasObject(By.text("Appearance")) ||
                device.hasObject(By.text("Font Size")) ||
                device.hasObject(By.text("Cursor style"))
        } catch (_: Exception) {
            device.hasObject(By.res("SettingsScreen")) || device.hasObject(By.text("Font Family"))
        }
    }
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
        view.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + 1200, MotionEvent.ACTION_MOVE, x + 1f, y + 1f, 0),
        )
        view.dispatchTouchEvent(
            MotionEvent.obtain(dt, dt + 1250, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0),
        )
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
