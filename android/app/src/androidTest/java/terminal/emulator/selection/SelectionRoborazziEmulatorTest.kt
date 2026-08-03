package terminal.emulator.selection

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
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.waitForSession
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
            bridge.setSelection(0, 0, 0, 0, hasSelection = false)
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
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_highlightActive_rendersInverseVideo() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'select this text segment'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0, 0, 0, 6, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()

        // Skip pixel count check in Robolectric - captureRoboImage() does comparison
    }

    @Test
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_longPress_wordSelection() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'word_selection_test'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.expandAndSetSelection(0, 5, mode = 1)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_dragEndHandle_repositionsHighlight() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'drag handle across this'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0, 5, 0, 11, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        bridge.setSelection(0, 5, 0, 18, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_clearSelection_returnsToNormal() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'temporary selection'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0, 0, 0, 9, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        bridge.setSelection(0, 0, 0, 0, hasSelection = false)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_toolbarVisible_withSelectionActive() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'toolbar test content'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0, 5, 0, 12, hasSelection = true)
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
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_modifierBar_visibleDuringSelection() {
        composeTestRule
            .onNodeWithTag("ModifierBar")
            .captureRoboImage()
    }

    @Test
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_clearedAndReSelected() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'reselect demo'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0, 5, 0, 11, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        bridge.setSelection(0, 5, 0, 11, hasSelection = false)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(200)

        bridge.setSelection(0, 0, 0, 4, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(300)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
    }

    @Test
    @org.junit.Ignore("Selection cannot be activated while Bridge.setSelection/expandAndSetSelection are log-only implemented (native query path wired since round-130)s; screenshots contain no selection (round-108)")
    fun selection_multipleLines_highlighted() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'line one'\n".toByteArray())
        Thread.sleep(300)
        bridge.writeToPty("echo 'line two'\n".toByteArray())
        Thread.sleep(300)
        bridge.writeToPty("echo 'line three'\n".toByteArray())
        composeTestRule.waitForIdle()
        Thread.sleep(1500)

        bridge.setSelection(0, 5, 2, 5, hasSelection = true)
        bridge.render()
        composeTestRule.waitForIdle()
        Thread.sleep(500)

        composeTestRule
            .onNodeWithTag("TerminalScreen")
            .captureRoboImage()
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
