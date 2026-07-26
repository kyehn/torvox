

package io.term.gpu

import android.graphics.Bitmap
import android.graphics.Color
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import io.term.MainActivity
import io.term.analyzeNonBlackRatio
import io.term.decodeRgbaToBitmap
import io.term.getBridge
import io.term.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.nio.ByteBuffer

class BackgroundImageGpuTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun background_image_solid_red_renders_non_black_pixels() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")

        val width = 100
        val height = 100
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(Color.RED)
        val buffer = ByteBuffer.allocate(width * height * 4)
        bitmap.copyPixelsToBuffer(buffer)
        bitmap.recycle()
        val rgbaData = buffer.array()

        bridge.setBackgroundImage(rgbaData, width.toUInt(), height.toUInt())
        bridge.setBackgroundParams(0u, 10u)

        kotlinx.coroutines.runBlocking {
            kotlinx.coroutines.delay(500)
        }

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("GPU frame must exist", frameFile.exists())

        val decodedBitmap = decodeRgbaToBitmap(frameFile)

        val ratio = analyzeNonBlackRatio(decodedBitmap)
        assertTrue(
            "Background image should cover significant portion (>5%) of screen, got $ratio",
            ratio > 0.05,
        )

        bridge.clearBackgroundImage()
    }

    @Test
    fun background_image_transform_scales_to_screen() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")

        val width = 16
        val height = 16
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(Color.RED)
        val buffer = ByteBuffer.allocate(width * height * 4)
        bitmap.copyPixelsToBuffer(buffer)
        bitmap.recycle()
        val rgbaData = buffer.array()

        bridge.setBackgroundImage(rgbaData, width.toUInt(), height.toUInt())
        bridge.setBackgroundParams(0u, 10u)

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val displayMetrics = context.resources.displayMetrics
        val screenWidth = displayMetrics.widthPixels
        val screenHeight = displayMetrics.heightPixels

        kotlinx.coroutines.runBlocking {
            kotlinx.coroutines.delay(500)
        }

        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("GPU frame must exist", frameFile.exists())

        val decodedBitmap = decodeRgbaToBitmap(frameFile)

        assertTrue(
            "Frame width ($screenWidth) should match frame bitmap width (${decodedBitmap.width})",
            decodedBitmap.width >= screenWidth * 0.9,
        )

        assertTrue(
            "Frame should have >= 5% coverage from background image",
            analyzeNonBlackRatio(decodedBitmap) > 0.05,
        )

        bridge.clearBackgroundImage()
    }

    @Test
    fun background_image_clear_restores_solid_theme() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")

        val width = 50
        val height = 50
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(Color.RED)
        val buffer = ByteBuffer.allocate(width * height * 4)
        bitmap.copyPixelsToBuffer(buffer)
        bitmap.recycle()
        val rgbaData = buffer.array()

        bridge.setBackgroundImage(rgbaData, width.toUInt(), height.toUInt())
        bridge.setBackgroundParams(0u, 10u)

        kotlinx.coroutines.runBlocking {
            kotlinx.coroutines.delay(500)
        }

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("GPU frame must exist", frameFile.exists())

        val beforeDecoded = decodeRgbaToBitmap(frameFile)
        val beforeRatio = analyzeNonBlackRatio(beforeDecoded)

        bridge.clearBackgroundImage()

        kotlinx.coroutines.runBlocking {
            kotlinx.coroutines.delay(500)
        }

        bridge.saveTestFrame(dataDir)
        val afterFile = File(dataDir, "test_frame.rgba")
        assertTrue("After frame must exist", afterFile.exists())

        val afterDecoded = decodeRgbaToBitmap(afterFile)
        val afterRatio = analyzeNonBlackRatio(afterDecoded)

        assertTrue(
            "After clearing, non-black ratio ($afterRatio) should be much less than before ($beforeRatio)",
            afterRatio < 0.01 || afterRatio < beforeRatio * 0.1,
        )

        bridge.clearBackgroundImage()
    }
}
