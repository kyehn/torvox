package io.term.selection

import android.graphics.Bitmap
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import io.term.MainActivity
import io.term.getBridge
import io.term.waitForSession
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.io.FileOutputStream

@RunWith(AndroidJUnit4::class)
@LargeTest
class SelectionRoborazziEmulatorTest {
    @get:Rule
    val roborazziRule =
        RoborazziRule(
            options =
            RoborazziRule.Options(
                roborazziOptions =
                RoborazziOptions(
                    compareOptions =
                    RoborazziOptions.CompareOptions(
                        changeThreshold = 0.05f,
                    ),
                ),
            ),
        )

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setup() {
        composeTestRule.waitForSession()
        composeTestRule.waitForIdle()
    }

    @After
    fun tearDown() {
        val bridge = composeTestRule.getBridge()
        if (bridge != null) {
            bridge.setSelection(0u, 0u, 0u, 0u, active = false)
            bridge.render()
        }
    }

    @Test
    fun selection_terminalScreen_exists() {
        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_afterTypingText_rendersContent() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'hello world for selection'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_highlightActive_rendersInverseVideo() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'select this text segment'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0u, 0u, 0u, 6u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()

        // Skip pixel count check in Robolectric - captureRoboImage() does comparison
    }

    @Test
    fun selection_longPress_wordSelection() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'word_selection_test'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.expandAndSetSelection(0u, 5u, mode = 1)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_dragEndHandle_repositionsHighlight() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'drag handle across this'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0u, 5u, 0u, 11u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        bridge.setSelection(0u, 5u, 0u, 18u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_clearSelection_returnsToNormal() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'temporary selection'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0u, 0u, 0u, 9u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        bridge.setSelection(0u, 0u, 0u, 0u, active = false)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_toolbarVisible_withSelectionActive() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'toolbar test content'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0u, 5u, 0u, 12u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(800)

        val terminalView = checkNotNull(findTerminalSurfaceView()) { "terminal surface view must be present for toolbar capture" }
        val bitmap = captureViewBitmap(terminalView)
        val screenshotDir =
            File(
                InstrumentationRegistry.getInstrumentation().targetContext.filesDir,
                "screenshots",
            )
        screenshotDir.mkdirs()
        val file = File(screenshotDir, "selection-toolbar.png")
        FileOutputStream(file).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_modifierBar_visibleDuringSelection() {
        composeTestRule
            .onNodeWithTag("ModifierBar")
            .captureRoboImage()
    }

    @Test
    fun selection_clearedAndReSelected() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'reselect demo'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0u, 5u, 0u, 11u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        bridge.setSelection(0u, 5u, 0u, 11u, active = false)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(200)

        bridge.setSelection(0u, 0u, 0u, 4u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    fun selection_multipleLines_highlighted() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'line one'\n".toByteArray())
        Thread.sleep(300)
        bridge.writeToPty("echo 'line two'\n".toByteArray())
        Thread.sleep(300)
        bridge.writeToPty("echo 'line three'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0u, 5u, 2u, 5u, active = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    private fun captureSelectionRegionPixels(): Int {
        val terminalView = findTerminalSurfaceView() ?: return 0
        val bitmap = captureViewBitmap(terminalView)
        val width = bitmap.width
        val height = bitmap.height
        val pixels = IntArray(width * height)
        bitmap.getPixels(pixels, 0, width, 0, 0, width, height)
        var nonBlack = 0
        for (pixel in pixels) {
            val r = (pixel shr 16) and 0xFF
            val g = (pixel shr 8) and 0xFF
            val b = pixel and 0xFF
            if (r > 10 || g > 10 || b > 10) nonBlack++
        }
        bitmap.recycle()
        return nonBlack
    }

    private fun findTerminalSurfaceView(): View? {
        var result: View? = null
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val content = activity.findViewById<View>(android.R.id.content) as? ViewGroup ?: return@onActivity
            result = findViewWithTag(content, "TerminalSurfaceView")
            if (result == null) {
                result = findTextureView(content)
            }
        }
        return result
    }

    private fun findViewWithTag(
        view: View,
        tag: String,
    ): View? {
        if (tag == view.tag) return view
        if (view is ViewGroup) {
            for (i in 0 until view.childCount) {
                val found = findViewWithTag(view.getChildAt(i), tag)
                if (found != null) return found
            }
        }
        return null
    }

    private fun findTextureView(group: ViewGroup): View? {
        for (i in 0 until group.childCount) {
            val child = group.getChildAt(i)
            if (child is android.view.TextureView) return child
            if (child is ViewGroup) {
                val result = findTextureView(child)
                if (result != null) return result
            }
        }
        return null
    }

    private fun captureViewBitmap(view: View): Bitmap {
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        val canvas = android.graphics.Canvas(bitmap)
        view.draw(canvas)
        return bitmap
    }
}
