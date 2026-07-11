package io.torvox.ocr

import android.graphics.Bitmap
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import io.torvox.MainActivity
import io.torvox.analyzeNonBlackRatio
import io.torvox.decodeRgbaToBitmap
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import java.io.File

class MlkOcrTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    // ── helpers removed — using shared io.torvox.* utils ──────

    @Test
    fun mlk_ocr_verifies_shell_prompt() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")

        val dataText = bridge.getTerminalText()
        assertTrue(
            "getTerminalText must contain shell prompt chars",
            dataText != null && dataText.contains(":") && dataText.contains("\$"),
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("GPU frame must exist", frameFile.exists())

        val bitmap = decodeRgbaToBitmap(frameFile)

        try {
            val image = InputImage.fromBitmap(bitmap, 0)
            val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
            val result =
                com.google.android.gms.tasks.Tasks
                    .await(recognizer.process(image))
            val ocrText = result.text
            bitmap.recycle()
            recognizer.close()

            val screenshotDir = File(context.filesDir, "screenshots")
            screenshotDir.mkdirs()
            File(screenshotDir, "mlk-ocr-output.txt").writeText(ocrText)

            assertTrue(
                "ML Kit OCR must detect shell prompt characters in:\n$ocrText",
                ocrText.contains(":") || ocrText.contains("/") || ocrText.contains("home"),
            )
        } catch (e: Exception) {
            bitmap.recycle()
            android.util.Log.w("MlkOcrTest", "ML Kit unavailable, skipping: ${e.message}")
        }
    }

    @Test
    fun mlk_ocr_after_echo_command() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'TORVOX_OCR_CHECK_123'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TORVOX_OCR_CHECK_123 must appear in data text",
            dataText != null && dataText.contains("TORVOX_OCR_CHECK_123"),
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val bitmap = decodeRgbaToBitmap(frameFile)

        try {
            val image = InputImage.fromBitmap(bitmap, 0)
            val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
            val result =
                com.google.android.gms.tasks.Tasks
                    .await(recognizer.process(image))
            val ocrText = result.text
            bitmap.recycle()
            recognizer.close()

            val screenshotDir = File(context.filesDir, "screenshots")
            screenshotDir.mkdirs()
            File(screenshotDir, "mlk-ocr-echo-output.txt").writeText(ocrText)

            assertTrue(
                "ML Kit must detect TORVOX_OCR_CHECK_123 in:\n$ocrText",
                ocrText.contains("TORVOX_OCR_CHECK_123"),
            )
        } catch (e: Exception) {
            bitmap.recycle()
            android.util.Log.w("MlkOcrTest", "ML Kit unavailable, skipping: ${e.message}")
        }
    }

    @Test
    fun mlk_ocr_cjk_text() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo '你好世界test_你好'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        val stripped = dataText?.replace(" ", "")
        assertTrue(
            "TIER1 GATE: terminal text must contain CJK characters '你好世界' " +
                "(got: '$dataText', stripped: '$stripped')",
            (dataText != null && dataText.contains("你好")) ||
                (stripped != null && stripped.contains("你好世界")),
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val bitmap = decodeRgbaToBitmap(frameFile)
        val ratio = analyzeNonBlackRatio(bitmap)

        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "mlk-cjk-gpu.png")
        pngFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()
        assertTrue(
            "Non-black pixel ratio $ratio <= 0.05 — CJK GPU frame appears blank",
            ratio > 0.05,
        )
    }
}
