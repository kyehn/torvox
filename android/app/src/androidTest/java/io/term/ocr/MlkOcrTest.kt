package io.term.ocr

import android.graphics.Bitmap
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import io.term.MainActivity
import io.term.analyzeNonBlackRatio
import io.term.decodeRgbaToBitmap
import io.term.getBridge
import io.term.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import java.io.File

class MlkOcrTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    // ── helpers removed — using shared io.term.* utils ──────

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

        requireMlKitAvailable(context)

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
    }

    private fun requireMlKitAvailable(context: android.content.Context) {
        val availability =
            com.google.android.gms.common.GoogleApiAvailability
                .getInstance()
                .isGooglePlayServicesAvailable(context)
        org.junit.Assert.assertEquals(
            "ML Kit OCR requires Google Play Services; the CI emulator image must provide GMS " +
                "(use a google_apis_playstore system image), not an AOSP image without GMS",
            com.google.android.gms.common.ConnectionResult.SUCCESS,
            availability,
        )
    }

    @Test
    fun mlk_ocr_after_echo_command() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'OCR_CHECK_123'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "OCR_CHECK_123 must appear in data text",
            dataText != null && dataText.contains("OCR_CHECK_123"),
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val bitmap = decodeRgbaToBitmap(frameFile)

        requireMlKitAvailable(context)

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
            "ML Kit must detect OCR_CHECK_123 in:\n$ocrText",
            ocrText.contains("OCR_CHECK_123"),
        )
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
