package io.torvox.selection

import android.content.Context
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import io.torvox.MainActivity
import io.torvox.PixelFrame
import io.torvox.assertSelectionMatches
import io.torvox.decodeRgbaToPixels
import io.torvox.extractCell
import io.torvox.getBridge
import io.torvox.matchConfidence
import io.torvox.saveAsPng
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import java.io.File

class SelectionGpuTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun captureFrame(dataDir: String): PixelFrame {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.saveTestFrame(dataDir)
        return decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
    }

    private fun captureFrameWithSelection(
        dataDir: String,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
    ): PixelFrame {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        // Pause the render thread so it does NOT overwrite the bridge selection
        // between setSelection and saveTestFrame.
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        bridge.setSelection(startRow.toUInt(), startCol.toUInt(), endRow.toUInt(), endCol.toUInt(), true)
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

    @Test
    fun selection_word_gpu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: terminal text must contain 'hello world'",
            dataText != null && dataText.contains("hello world"),
        )

        val templateFrame = captureFrame(dataDir)
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
        assertTrue(
            "TIER1 GATE: 'world' must be found in terminal output",
            echoLine >= 0 && colOfWorld >= 0,
        )

        val selectionFrame =
            captureFrameWithSelection(
                dataDir,
                echoLine,
                colOfWorld,
                echoLine,
                colOfWorld + 5,
            )

        assertSelectionMatches(
            templateFrame,
            selectionFrame,
            gridCols,
            gridRows,
            echoLine,
            colOfWorld..colOfWorld + 4,
        )

        val screenshotDir = File(context.filesDir, "screenshots")
        saveEvidence(templateFrame, "word-template", screenshotDir)
        saveEvidence(selectionFrame, "word-selection", screenshotDir)
    }

    @Test
    fun selection_line_gpu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: terminal text must contain 'hello world'",
            dataText != null && dataText.contains("hello world"),
        )

        val templateFrame = captureFrame(dataDir)
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
        assertTrue(
            "TIER1 GATE: a line with 'hello world' must be found",
            echoLine >= 0,
        )

        val lineLen = bridge.getTerminalText()!!.split("\n")[echoLine].length
        val selectionFrame =
            captureFrameWithSelection(
                dataDir,
                echoLine,
                0,
                echoLine,
                (lineLen - 1).coerceAtLeast(0),
            )

        for (col in 0 until gridCols) {
            val actual = extractCell(selectionFrame, col, echoLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, echoLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue(
                "Line selection: cell ($echoLine,$col) confidence $c should be < 0.5",
                c < 0.5,
            )
        }

        val adjRow = if (echoLine > 0) echoLine - 1 else echoLine + 1
        if (adjRow in 0 until gridRows) {
            for (col in 0 until gridCols) {
                val actual = extractCell(selectionFrame, col, adjRow, gridCols, gridRows)
                val tmpl = extractCell(templateFrame, col, adjRow, gridCols, gridRows)
                val c = matchConfidence(actual, tmpl)
                assertTrue(
                    "Line selection: adjacent row cell ($adjRow,$col) confidence $c should be >= 0.9",
                    c >= 0.9,
                )
            }
        }

        val screenshotDir = File(context.filesDir, "screenshots")
        saveEvidence(templateFrame, "line-template", screenshotDir)
        saveEvidence(selectionFrame, "line-selection", screenshotDir)
    }

    @Test
    fun selection_no_active_gpu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.writeToPty("echo 'A'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: terminal text must contain 'A'",
            dataText != null && dataText.contains("A"),
        )

        val templateFrame = captureFrame(dataDir)
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var echoLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("A")) {
                echoLine = i
                break
            }
        }
        assertTrue("TIER1 GATE: line with 'A' must be found", echoLine >= 0)

        val bridge2 = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        bridge2.setSelection(echoLine.toUInt(), 5u, echoLine.toUInt(), 5u, false)
        bridge2.saveTestFrame(dataDir)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        val selectionFrame = decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))

        for (col in 0 until gridCols) {
            val actual = extractCell(selectionFrame, col, echoLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, echoLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue(
                "Inactive selection: cell ($echoLine,$col) confidence $c should be >= 0.95",
                c >= 0.95,
            )
        }

        val screenshotDir = File(context.filesDir, "screenshots")
        saveEvidence(templateFrame, "inactive-template", screenshotDir)
        saveEvidence(selectionFrame, "inactive-selection", screenshotDir)
    }

    @Test
    fun selection_clear_gpu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: terminal text must contain 'hello world'",
            dataText != null && dataText.contains("hello world"),
        )

        val templateFrame = captureFrame(dataDir)
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
        assertTrue(
            "TIER1 GATE: 'world' must be found in terminal output",
            echoLine >= 0 && colOfWorld >= 0,
        )

        val selectionFrame =
            captureFrameWithSelection(
                dataDir,
                echoLine,
                colOfWorld,
                echoLine,
                colOfWorld + 5,
            )

        assertSelectionMatches(
            templateFrame,
            selectionFrame,
            gridCols,
            gridRows,
            echoLine,
            colOfWorld..colOfWorld + 4,
        )

        val bridge2 = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(500)
        bridge2.setSelection(0u, 0u, 0u, 0u, false)
        bridge2.saveTestFrame(dataDir)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        val postClearFrame = decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))

        for (col in 0 until gridCols) {
            val actual = extractCell(postClearFrame, col, echoLine, gridCols, gridRows)
            val tmpl = extractCell(templateFrame, col, echoLine, gridCols, gridRows)
            val c = matchConfidence(actual, tmpl)
            assertTrue(
                "Cleared: cell ($echoLine,$col) confidence $c should be >= 0.95",
                c >= 0.95,
            )
        }

        val screenshotDir = File(context.filesDir, "screenshots")
        saveEvidence(templateFrame, "clear-template", screenshotDir)
        saveEvidence(selectionFrame, "clear-selection-active", screenshotDir)
        saveEvidence(postClearFrame, "clear-post", screenshotDir)
    }
}
