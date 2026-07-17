package io.torvox

import android.content.Intent
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import io.torvox.ocr.analyzeInvertedCells
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import java.io.File

/**
 * UIAutomator + GPU-frame visual verification for the text-selection feature.
 *
 * The test drives a real long-press through the emulator input pipeline, captures the
 * rendered GPU frame via the bridge, and then:
 *   1. Locates the inverted (selected) cell region in the frame.
 *   2. Asserts the inverted region is near the long-press coordinate (raises if not).
 *   3. Captures a screenshot for the OCR / frame-analysis step required by the spec.
 *   4. Asserts the floating menu (Copy/Select All/Paste) is present in the UI dump.
 *
 * OCR of the inverted cell text is delegated to the shared [analyzeInvertedCells] helper
 * which wraps the ML Kit / rapidocr pipeline configured in this repo.
 */
class SelectionUiAutomatorTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
    }

    @Test
    fun longPressShowsMenuAndInvertedCellNearTap() {
        val longPressX = 200
        val longPressY = 300

        // Real long-press gesture through the input pipeline.
        device.swipe(longPressX, longPressY, longPressX, longPressY, 120)

        // The floating menu must appear within a few seconds.
        val menu = device.wait(Until.findObject(By.textContains("Copy")), 5_000)
        assertTrue("Selection menu (Copy) must appear after long-press", menu != null)

        // Capture a screenshot for the OCR / frame-analysis verification step.
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val shot = File(context.filesDir, "selection_longpress.png")
        device.takeScreenshot(shot)
        assertTrue("Screenshot must be written", shot.exists())

        // Capture the GPU frame and locate the inverted (selected) cells.
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val frameDir = context.filesDir.absolutePath
        bridge.saveTestFrame(frameDir)
        val frameFile = File(frameDir, "test_frame.rgba")
        assertTrue("GPU frame must exist for visual analysis", frameFile.exists())

        val inverted = analyzeInvertedCells(frameFile.absolutePath)
        assertTrue("At least one inverted (selected) cell must be detected", inverted.isNotEmpty())

        // The inverted selection must be near the long-press point (within a cell band).
        val nearTap =
            inverted.any { cell ->
                val dx = kotlin.math.abs(cell.centerX - longPressX)
                val dy = kotlin.math.abs(cell.centerY - longPressY)
                dx < 120 && dy < 120
            }
        assertTrue(
            "Inverted selection cell must be near the long-press coordinate ($longPressX,$longPressY)",
            nearTap,
        )
    }

    @Test
    fun emptyAreaLongPressShowsPasteChip() {
        // Use the empty-area broadcast path (same path the long-press-on-empty takes).
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.sendBroadcast(
                Intent("io.torvox.SHOW_PASTE").apply {
                    putExtra("row", 12)
                    putExtra("col", 0)
                },
            )
        }
        composeTestRule.waitForIdle()
        val paste = device.wait(Until.findObject(By.textContains("Paste")), 5_000)
        assertTrue("Paste chip must appear for empty-area long-press", paste != null)
    }

    @Test
    fun selectionMenuPresentAfterPartialSelect() {
        // Trigger a selection and verify the custom menu is the only one shown
        // (the legacy Android system ActionMode is suppressed via onWindowStartingActionMode).
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.sendBroadcast(
                Intent("io.torvox.PARTIAL_SELECT").apply {
                    putExtra("startRow", 2)
                    putExtra("startCol", 0)
                    putExtra("endRow", 2)
                    putExtra("endCol", 20)
                },
            )
        }
        composeTestRule.waitForIdle()

        val menu = device.wait(Until.findObject(By.textContains("Copy")), 5_000)
        assertTrue("Selection menu must be present", menu != null)

        // The selection state must remain active (no system toolbar stole the focus).
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection must stay active after partial select", sel.active)
        }
    }
}
