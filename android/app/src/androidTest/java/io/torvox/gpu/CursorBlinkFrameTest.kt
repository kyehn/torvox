package io.torvox.gpu

import android.content.Context
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import io.torvox.MainActivity
import io.torvox.decodeRgbaToPixels
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Assert
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import java.io.File

class CursorBlinkFrameTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun cursorBlink_causesPixelChangesBetweenFrames() {
        val context: Context = composeTestRule.activity.applicationContext
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

        val testDir = File(context.cacheDir, "blink_test_${System.nanoTime()}")
        testDir.mkdirs()

        Thread.sleep(500)

        bridge.saveTestFrame(testDir.absolutePath)
        Thread.sleep(700)
        bridge.saveTestFrame(testDir.absolutePath)

        val files = testDir.listFiles { f -> f.name.endsWith(".rgba") && f.name.startsWith("frame_") }
        Assert.assertNotNull("No frame files written", files)
        Assert.assertTrue("Need at least 2 frame files", files != null && files.size >= 2)

        val sorted = files.sortedBy { it.lastModified() }
        val frame1 = decodeRgbaToPixels(sorted[sorted.size - 2])
        val frame2 = decodeRgbaToPixels(sorted.last())

        Assert.assertEquals("Frame dimensions must match", frame1.pixels.size, frame2.pixels.size)

        val changedPixels = frame1.pixels.zip(frame2.pixels).count { (a, b) -> a != b }
        Assert.assertTrue(
            "Expected pixel changes from cursor blink (found $changedPixels / ${frame1.pixels.size})",
            changedPixels > 0,
        )

        testDir.deleteRecursively()
    }
}
