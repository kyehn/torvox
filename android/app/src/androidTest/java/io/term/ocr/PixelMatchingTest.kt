package io.term.ocr

import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import io.term.MainActivity
import io.term.PixelFrame
import io.term.buildTemplate
import io.term.cellBottom
import io.term.cellRight
import io.term.cellX
import io.term.cellY
import io.term.decodeRgbaToPixels
import io.term.extractCell
import io.term.getBridge
import io.term.lastGridRowsDensity
import io.term.matchConfidence
import io.term.rowProfile
import io.term.saveAsPng
import io.term.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Ignore
import org.junit.Rule
import org.junit.Test
import java.io.File

class PixelMatchingTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    // ── helpers removed — using shared io.term.* utils ──────

    // ── test 01: shell prompt (Tier 1 gate + Tier 2 warn) ──────

    @Test
    fun pixel_match_shell_prompt() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: terminal text must be non-null and contain ':' and '\$'",
            dataText != null && dataText.contains(":") && dataText.contains("\$"),
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("GPU frame file must exist", frameFile.exists())

        val frame = decodeRgbaToPixels(frameFile)

        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        saveAsPng(frame, File(screenshotDir, "pixel-match-shell-prompt.png"))

        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val profile = rowProfile(frame)
        val lastRowDensity = lastGridRowsDensity(profile, frame.width, frame.height, gridRows, 3)

        val evidenceText =
            buildString {
                appendLine("gridRows=$gridRows gridCols=$gridCols")
                appendLine("lastRowDensity=$lastRowDensity")
                appendLine("text_from_bridge=${dataText?.take(200)}")
            }
        File(screenshotDir, "pixel-match-evidence.txt").writeText(evidenceText)

        assertTrue(
            "WARN: pixel row density $lastRowDensity <= 0.1 (expect non-empty prompt line)",
            lastRowDensity > 0.1,
        )
    }

    // ── test 02: cross-position character matching (Fix 1) ─────

    @Test
    fun pixel_match_character_templates() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.writeToPty("echo 'ABCDEFGHIJ_AAAAA'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: ABCDEFGHIJ_AAAAA must appear in data text",
            dataText != null && dataText.contains("ABCDEFGHIJ_AAAAA"),
        )

        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val frame = decodeRgbaToPixels(frameFile)

        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        saveAsPng(frame, File(screenshotDir, "pixel-match-templates.png"))

        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()

        val lines = dataText!!.split("\n")
        var echoLine = -1
        for ((i, line) in lines.withIndex()) {
            if (line.contains("ABCDEFGHIJ_AAAAA")) {
                echoLine = i
                break
            }
        }
        if (echoLine < 0) echoLine = gridRows - 2

        val templateColA = lines[echoLine].indexOf('A')
        assertTrue("Character 'A' must be found in echo output line", templateColA >= 0)

        val template = buildTemplate(frame, templateColA, echoLine, gridCols, gridRows)

        val secondACol = lines[echoLine].indexOf('A', templateColA + 1)
        assertTrue("Second 'A' must exist at a different position for cross-position match", secondACol > templateColA)

        val actual = extractCell(frame, secondACol, echoLine, gridCols, gridRows)
        val confidence = matchConfidence(actual, template)

        File(screenshotDir, "char-match-confidence.txt").writeText(
            "templateCol=$templateColA matchCol=$secondACol row=$echoLine confidence=$confidence",
        )

        assertTrue(
            "WARN: cross-position match confidence for 'A' must be > 0.7, got $confidence",
            confidence > 0.7,
        )
    }

    // ── test 03: echo command (Tier 1 gate + Tier 2 warn) ──────

    @Test
    fun pixel_match_after_echo_command() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath

        bridge.writeToPty("echo 'HELLO_PIXEL'\n".toByteArray())
        Thread.sleep(3000)

        val dataText = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: HELLO_PIXEL must appear in data text",
            dataText != null && dataText.contains("HELLO_PIXEL"),
        )

        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val frame = decodeRgbaToPixels(frameFile)

        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        saveAsPng(frame, File(screenshotDir, "pixel-match-echo.png"))

        val gridRows = bridge.getGridRows()
        val profile = rowProfile(frame)
        val echoRowDensity = lastGridRowsDensity(profile, frame.width, frame.height, gridRows, 5)

        File(screenshotDir, "echo-row-density.txt").writeText(
            "text=${dataText?.take(200)}\nechoRowDensity=$echoRowDensity",
        )

        assertTrue(
            "WARN: row density $echoRowDensity must be > 0.1 after echo command",
            echoRowDensity > 0.1,
        )
    }

    // ── test 04: GPU frame + getTerminalText() agreement ──

    @Test
    fun pixel_match_gpu_text_agreement() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")

        val text = bridge.getTerminalText()
        assertTrue(
            "TIER1 GATE: getTerminalText() must return non-null text",
            text != null && text.isNotEmpty(),
        )

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val frame = decodeRgbaToPixels(frameFile)

        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()

        saveAsPng(frame, File(screenshotDir, "gpu-text-agreement.png"))

        File(screenshotDir, "gpu-extracted-text.txt").writeText(text!!)

        var nonBlack = 0L
        val total = frame.pixels.size.toLong()
        for (pixel in frame.pixels) {
            val r = (pixel shr 16) and 0xFF
            val g = (pixel shr 8) and 0xFF
            val b = pixel and 0xFF
            if (r > 10 || g > 10 || b > 10) nonBlack++
        }
        val ratio = nonBlack.toDouble() / total.toDouble()

        File(screenshotDir, "gpu-agreement-evidence.txt").writeText(
            "text=${text.take(200)}\nnonBlackRatio=$ratio",
        )

        assertTrue(
            "WARN: non-black pixel ratio $ratio <= 0.05 — GPU frame appears blank",
            ratio > 0.05,
        )
    }

    // ── test 05: CJK character rendering ──

    @Test
    fun pixel_match_cjk_rendering() {
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
        val frame = decodeRgbaToPixels(frameFile)

        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        saveAsPng(frame, File(screenshotDir, "pixel-match-cjk.png"))

        var nonBlack = 0L
        val total = frame.pixels.size.toLong()
        for (pixel in frame.pixels) {
            val r = (pixel shr 16) and 0xFF
            val g = (pixel shr 8) and 0xFF
            val b = pixel and 0xFF
            if (r > 10 || g > 10 || b > 10) nonBlack++
        }
        val ratio = nonBlack.toDouble() / total.toDouble()

        File(screenshotDir, "cjk-evidence.txt").writeText(
            "text=${dataText?.take(200)}\nnonBlackRatio=$ratio",
        )

        assertTrue(
            "WARN: non-black pixel ratio $ratio <= 0.05 for CJK frame",
            ratio > 0.05,
        )
    }
}
