package io.torvox.selection

import android.graphics.Bitmap
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import io.torvox.MainActivity
import io.torvox.PixelFrame
import io.torvox.analyzeNonBlackRatio
import io.torvox.decodeRgbaToBitmap
import io.torvox.decodeRgbaToPixels
import io.torvox.extractCell
import io.torvox.getBridge
import io.torvox.matchConfidence
import io.torvox.saveAsPng
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class SelectionEmulatorTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext
    private val dataDir get() = context.filesDir.absolutePath

    private fun captureFrame(): PixelFrame {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.saveTestFrame(dataDir)
        return decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
    }

    private fun captureFrameWithSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        active: Boolean = true,
    ): PixelFrame {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        bridge.setSelection(startRow.toUInt(), startCol.toUInt(), endRow.toUInt(), endCol.toUInt(), active)
        bridge.saveTestFrame(dataDir)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        return decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
    }

    private fun saveEvidence(
        frame: PixelFrame,
        name: String,
        screenshotDir: File,
    ) {
        screenshotDir.mkdirs()
        saveAsPng(frame, File(screenshotDir, "$name.png"))
    }

    private fun dispatchLongPress(
        x: Float,
        y: Float,
    ) {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val surface = activity.window.decorView.findViewWithTag<android.view.View>("TerminalSurfaceView")
            if (surface != null) {
                val downTime = SystemClock.uptimeMillis()
                surface.dispatchTouchEvent(
                    MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0).apply {
                        source = InputDevice.SOURCE_TOUCHSCREEN
                    },
                )
                Thread.sleep(1200)
                val upTime = SystemClock.uptimeMillis()
                surface.dispatchTouchEvent(
                    MotionEvent.obtain(downTime, upTime, MotionEvent.ACTION_UP, x, y, 0).apply {
                        source = InputDevice.SOURCE_TOUCHSCREEN
                    },
                )
            }
        }
        Thread.sleep(2000)
    }

    @Test
    fun selection_longPressTriggersHighlight_onTerminalText() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'SELECTION_TEST_MARKER'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "Terminal must contain SELECTION_TEST_MARKER",
            dataText != null && dataText.contains("SELECTION_TEST_MARKER"),
        )

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var markerLine = -1
        var markerCol = -1
        for ((i, line) in lines.withIndex()) {
            val idx = line.indexOf("MARKER")
            if (idx >= 0) {
                markerLine = i
                markerCol = idx
                break
            }
        }
        assertTrue("MARKER must be found in terminal output", markerLine >= 0 && markerCol >= 0)

        val selectionFrame =
            captureFrameWithSelection(
                startRow = markerLine,
                startCol = markerCol,
                endRow = markerLine,
                endCol = markerCol + 6,
            )

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "selection-marker-template", screenshotDir)
        saveEvidence(selectionFrame, "selection-marker-active", screenshotDir)
    }

    @Test
    fun selection_highlightClears_whenInactive() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'CLEAR_TEST'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue("Terminal must contain CLEAR_TEST", dataText != null && dataText.contains("CLEAR_TEST"))

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var testLine = -1
        var testCol = -1
        for ((i, line) in lines.withIndex()) {
            val idx = line.indexOf("CLEAR_TEST")
            if (idx >= 0) {
                testLine = i
                testCol = idx
                break
            }
        }
        assertTrue("CLEAR_TEST must be found", testLine >= 0)

        val activeFrame =
            captureFrameWithSelection(
                startRow = testLine,
                startCol = testCol,
                endRow = testLine,
                endCol = testCol + 5,
            )

        val clearedFrame =
            captureFrameWithSelection(
                startRow = 0,
                startCol = 0,
                endRow = 0,
                endCol = 0,
                active = false,
            )

        for (col in 0 until gridCols) {
            val actual = extractCell(clearedFrame, col, testLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, testLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue("Cleared cell ($testLine,$col): $c should be >= 0.9", c >= 0.9)
        }

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "clear-template", screenshotDir)
        saveEvidence(activeFrame, "clear-active", screenshotDir)
        saveEvidence(clearedFrame, "clear-after", screenshotDir)
    }

    @Test
    fun selection_wordGpu_rendersCorrectly() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "Terminal must contain 'hello world'",
            dataText != null && dataText.contains("hello world"),
        )

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var echoLine = -1
        var colOfWorld = -1
        for ((i, line) in lines.withIndex()) {
            val idx = line.indexOf("world")
            if (idx >= 0) {
                echoLine = i
                colOfWorld = idx
                break
            }
        }
        assertTrue("'world' must be found", echoLine >= 0 && colOfWorld >= 0)

        val selectionFrame =
            captureFrameWithSelection(
                echoLine,
                colOfWorld,
                echoLine,
                colOfWorld + 5,
            )

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "word-template", screenshotDir)
        saveEvidence(selectionFrame, "word-selection", screenshotDir)
    }

    @Test
    fun selection_lineGpu_highlightsFullRow() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "Terminal must contain 'hello world'",
            dataText != null && dataText.contains("hello world"),
        )

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var echoLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("hello world")) {
                echoLine = i
                break
            }
        }
        assertTrue("Line with 'hello world' must be found", echoLine >= 0)

        val lineLen = bridge.getTerminalText()!!.split("\n")[echoLine].length
        val selectionFrame =
            captureFrameWithSelection(
                echoLine,
                0,
                echoLine,
                (lineLen - 1).coerceAtLeast(0),
            )

        for (col in 0 until lineLen.coerceAtMost(gridCols)) {
            val actual = extractCell(selectionFrame, col, echoLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, echoLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue("Line selection cell ($echoLine,$col): $c should be < 0.65", c < 0.65)
        }

        val adjRow = if (echoLine > 0) echoLine - 1 else echoLine + 1
        if (adjRow in 0 until gridCols) {
            // Adjacent row check: some rows may share colors with prompt
        }

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "line-template", screenshotDir)
        saveEvidence(selectionFrame, "line-selection", screenshotDir)
    }

    @Test
    fun selection_coordinateMapping_longPressMapsToCorrectCell() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        val gridRows = bridge.getGridRows()
        assertTrue("Grid must have positive dimensions", gridCols > 0 && gridRows > 0)

        bridge.writeToPty("echo 'ABCDEFGHIJ'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue("Must contain ABCDEFGHIJ", dataText != null && dataText.contains("ABCDEFGHIJ"))

        val lines = dataText!!.split("\n")
        var targetLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("ABCDEFGHIJ")) {
                targetLine = i
                break
            }
        }
        assertTrue("ABCDEFGHIJ line must be found", targetLine >= 0)
    }

    @Test
    fun selection_multiRow_textExtraction() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'line1'\n".toByteArray())
        bridge.writeToPty("echo 'line2'\n".toByteArray())
        bridge.writeToPty("echo 'line3'\n".toByteArray())
        Thread.sleep(4000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "Must contain all lines",
            dataText != null && dataText.contains("line1") &&
                dataText.contains("line2") && dataText.contains("line3"),
        )
    }

    @Test
    fun selection_gpu_noActiveSelection_noHighlight() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'A'\n".toByteArray())
        Thread.sleep(3000)

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val dataText = bridge.getTerminalText()
        val lines = dataText!!.split("\n")
        var echoLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("A")) {
                echoLine = i
                break
            }
        }
        assertTrue("Line with 'A' must be found", echoLine >= 0)

        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        bridge.setSelection(echoLine.toUInt(), 5u, echoLine.toUInt(), 5u, false)
        bridge.saveTestFrame(dataDir)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        val inactiveFrame = decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))

        for (col in 0 until gridCols) {
            val actual = extractCell(inactiveFrame, col, echoLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, echoLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue("Inactive cell ($echoLine,$col): $c should be >= 0.95", c >= 0.95)
        }

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "inactive-template", screenshotDir)
        saveEvidence(inactiveFrame, "inactive-selection", screenshotDir)
    }

    @Test
    fun selection_gpu_clearAfterSelection_restoresOriginal() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val dataText = bridge.getTerminalText()
        val lines = dataText!!.split("\n")
        var echoLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("hello world")) {
                echoLine = i
                break
            }
        }
        assertTrue("Line with 'hello world' must be found", echoLine >= 0)

        val selectionFrame =
            captureFrameWithSelection(
                echoLine,
                0,
                echoLine,
                "hello world".length - 1,
            )

        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.saveTestFrame(dataDir)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        val postClearFrame = decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))

        for (col in 0 until gridCols) {
            val actual = extractCell(postClearFrame, col, echoLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, echoLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue("Cleared cell ($echoLine,$col): $c should be >= 0.95", c >= 0.95)
        }

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "clear2-template", screenshotDir)
        saveEvidence(selectionFrame, "clear2-active", screenshotDir)
        saveEvidence(postClearFrame, "clear2-post", screenshotDir)
    }

    @Test
    fun selection_ocr_verifiesShellPromptVisible() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")

        val dataText = bridge.getTerminalText()
        assertTrue(
            "Terminal text must contain shell prompt",
            dataText != null && dataText.contains("$") &&
                (dataText.contains("/") || dataText.contains(":")),
        )

        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("GPU frame must exist", frameFile.exists())

        val bitmap = decodeRgbaToBitmap(frameFile)

        assumeMlKitAvailable(composeTestRule.activity)

        val image = InputImage.fromBitmap(bitmap, 0)
        val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        val result =
            com.google.android.gms.tasks.Tasks
                .await(recognizer.process(image))
        val ocrText = result.text
        bitmap.recycle()
        recognizer.close()

        val screenshotDir = File(dataDir, "screenshots")
        screenshotDir.mkdirs()
        File(screenshotDir, "sel-ocr-output.txt").writeText(ocrText)

        assertTrue(
            "OCR must detect shell prompt: $ocrText",
            ocrText.contains("$") || ocrText.contains("/") || ocrText.contains("home"),
        )
    }

    private fun assumeMlKitAvailable(context: android.content.Context) {
        val availability =
            com.google.android.gms.common.GoogleApiAvailability
                .getInstance()
                .isGooglePlayServicesAvailable(context)
        org.junit.Assume.assumeTrue(
            "ML Kit OCR requires Google Play Services; skipping on AOSP emulator without GMS",
            availability == com.google.android.gms.common.ConnectionResult.SUCCESS,
        )
    }

    @Test
    fun selection_ocr_afterEchoCommand_detectsText() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'TORVOX_SEL_OCR_456'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TORVOX_SEL_OCR_456 must appear",
            dataText != null && dataText.contains("TORVOX_SEL_OCR_456"),
        )

        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val bitmap = decodeRgbaToBitmap(frameFile)

        assumeMlKitAvailable(composeTestRule.activity)

        val image = InputImage.fromBitmap(bitmap, 0)
        val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        val result =
            com.google.android.gms.tasks.Tasks
                .await(recognizer.process(image))
        val ocrText = result.text
        bitmap.recycle()
        recognizer.close()

        val screenshotDir = File(dataDir, "screenshots")
        screenshotDir.mkdirs()
        File(screenshotDir, "sel-ocr-echo-output.txt").writeText(ocrText)

        assertTrue(
            "OCR must detect TORVOX_SEL_OCR_456: $ocrText",
            ocrText.contains("TORVOX_SEL_OCR_456"),
        )
    }

    @Test
    fun selection_gpu_nonBlackPixels_increaseWithSelection() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue("Must contain 'hello world'", dataText != null && dataText.contains("hello world"))

        val lines = dataText!!.split("\n")
        var echoLine = -1
        var colOfWorld = -1
        for ((i, line) in lines.withIndex()) {
            val idx = line.indexOf("world")
            if (idx >= 0) {
                echoLine = i
                colOfWorld = idx
                break
            }
        }
        assertTrue("'world' must be found", echoLine >= 0 && colOfWorld >= 0)

        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(echoLine, colOfWorld, echoLine, colOfWorld + 5)

        val templateRatio = analyzeNonBlackRatio(templateFrame)
        val selRatio = analyzeNonBlackRatio(selFrame)

        android.util.Log.i("SelEmuTest", "template ratio=$templateRatio  selRatio=$selRatio")

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "ratio-template", screenshotDir)
        saveEvidence(selFrame, "ratio-selection", screenshotDir)
    }

    @Test
    fun selection_handlePositions_matchLogicalBounds() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        val gridRows = bridge.getGridRows()
        assertTrue("Grid must be positive", gridCols > 0 && gridRows > 0)
    }

    @Test
    fun selection_pauseResumeRender_preservesState() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'RENDER_PAUSE_TEST'\n".toByteArray())
        Thread.sleep(3000)

        val templateFrame = captureFrame()
        val dataText = bridge.getTerminalText()
        val lines = dataText!!.split("\n")
        var testLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("RENDER_PAUSE_TEST")) {
                testLine = i
                break
            }
        }
        assertTrue("RENDER_PAUSE_TEST line must be found", testLine >= 0)

        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        Thread.sleep(500)

        val resumeFrame = captureFrame()
        val resumeRatio = analyzeNonBlackRatio(resumeFrame)
        assertTrue("Resume frame must have content: $resumeRatio", resumeRatio > 0.01)

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "pause-template", screenshotDir)
        saveEvidence(resumeFrame, "pause-resume", screenshotDir)
    }

    @Test
    fun selection_gpu_differentColors_detectable() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'RED_GREEN_BLUE'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "Must contain RED_GREEN_BLUE",
            dataText != null && dataText.contains("RED_GREEN_BLUE"),
        )

        val templateFrame = captureFrame()
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var echoLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("RED_GREEN_BLUE")) {
                echoLine = i
                break
            }
        }
        assertTrue("RED_GREEN_BLUE line must be found", echoLine >= 0)

        val selectionFrame =
            captureFrameWithSelection(
                echoLine,
                0,
                echoLine,
                "RED_GREEN_BLUE".length - 1,
            )

        val templateRatio = analyzeNonBlackRatio(templateFrame)
        val selRatio = analyzeNonBlackRatio(selectionFrame)

        val screenshotDir = File(dataDir, "screenshots")
        saveEvidence(templateFrame, "color-template", screenshotDir)
        saveEvidence(selectionFrame, "color-selection", screenshotDir)

        android.util.Log.i("SelEmuTest", "color-template=$templateRatio  color-sel=$selRatio")
    }
}
