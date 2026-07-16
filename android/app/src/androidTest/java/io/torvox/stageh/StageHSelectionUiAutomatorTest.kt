package io.torvox.stageh

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * Stage H — UIAutomator-driven verification of the text-selection feature on the default
 * (session 1) terminal.
 *
 * Performs a real long-press on the terminal body (away from the status bar and modifier bar
 * so no system gesture is triggered), asserts the selection overlay / context menu appears
 * ("Paste" / "Copy" / "Select All"), and captures a screenshot of the selection state to
 * the app's file storage for later pull + pixel verification of the selection handle.
 *
 * Runs on the single default session only (no New Session / session switching).
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class StageHSelectionUiAutomatorTest {
    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.pressHome()

        val context = InstrumentationRegistry.getInstrumentation().context
        val intent = context.packageManager.getLaunchIntentForPackage("com.termux")
        assertNotNull("Launch intent for com.termux must exist", intent)
        context.startActivity(intent)
        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 20000)
        // Allow bootstrap + first render to settle.
        device.waitForIdle(8000)
    }

    @Test
    fun longPressShowsSelectionMenu_andHandle() {
        val terminal = device.findObject(By.desc("Terminal"))
        assertNotNull("Terminal surface must exist", terminal)

        // Long-press in the terminal body. Using the element center keeps the gesture away
        // from the status bar (top) and the modifier bar (bottom), avoiding system gestures.
        val bounds = terminal.visibleBounds
        val x = bounds.centerX()
        val y = bounds.centerY()
        device.swipe(x, y, x, y, 800)

        // The selection overlay / context menu must appear with Copy / Select All / Paste.
        val menu =
            device.wait(Until.findObject(By.textContains("Paste")), 5000)
                ?: device.wait(Until.findObject(By.textContains("Copy")), 5000)
                ?: device.wait(Until.findObject(By.textContains("Select")), 5000)
        assertNotNull(
            "Selection overlay/context menu (Paste/Copy/Select All) must appear after long-press",
            menu,
        )

        // Capture a screenshot of the selection state for offline handle verification.
        val targetContext = InstrumentationRegistry.getInstrumentation().targetContext
        val screenshot = File(targetContext.filesDir, "stageh_selection.png")
        assertTrue(
            "Screenshot of selection state must be captured",
            device.takeScreenshot(screenshot),
        )
        assertTrue("Screenshot file must be non-empty", screenshot.exists() && screenshot.length() > 0)
    }
}
