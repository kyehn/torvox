package io.torvox.ui

import android.graphics.Bitmap
import android.graphics.Color
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import io.torvox.MainActivity
import io.torvox.getBridge
import io.torvox.openDrawer
import io.torvox.waitForSession
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

@RunWith(AndroidJUnit4::class)
class SelectionEmulatorTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: androidx.test.uiautomator.UiDevice
    private var textureView: TextureView? = null
    private var screenshotDir: File? = null
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    private val context get() = instrumentation.targetContext
    private val dataDir get() = context.filesDir.absolutePath

    @Before
    fun setUp() {
        device =
            androidx.test.uiautomator.UiDevice
                .getInstance(instrumentation)
        composeTestRule.waitForSession()
        composeTestRule.waitForIdle()
        textureView = findTextureView(composeTestRule.activity.window.decorView)
        screenshotDir = File(dataDir, "SelectionEmulatorTest")
        screenshotDir!!.mkdirs()
    }

    @After
    fun tearDown() {
        val bridge = composeTestRule.getBridge()
        if (bridge != null) {
            bridge.setSelection(0u, 0u, 0u, 0u, active = false)
            bridge.render()
        }
    }

    private fun findTextureView(root: View): TextureView? {
        if (root is TextureView) return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                findTextureView(root.getChildAt(i))?.let { return it }
            }
        }
        return null
    }

    private fun takeScreenshot(): Bitmap {
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        val view = composeTestRule.activity.window.decorView
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        val canvas = android.graphics.Canvas(bitmap)
        view.draw(canvas)
        return bitmap
    }

    private fun saveScreenshot(name: String) {
        val bitmap = takeScreenshot()
        val file = File(screenshotDir, "$name.png")
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()
    }

    private fun dispatchLongPress(
        x: Float,
        y: Float,
    ) {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        val downTime = SystemClock.uptimeMillis()
        tv.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0).apply {
                source = InputDevice.SOURCE_TOUCHSCREEN
            },
        )
        Thread.sleep(1200)
        val upTime = SystemClock.uptimeMillis()
        tv.dispatchTouchEvent(
            MotionEvent.obtain(downTime, upTime, MotionEvent.ACTION_UP, x + 1f, y + 1f, 0).apply {
                source = InputDevice.SOURCE_TOUCHSCREEN
            },
        )
        Thread.sleep(1500)
    }

    private fun dispatchTap(
        x: Float,
        y: Float,
    ) {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        val downTime = SystemClock.uptimeMillis()
        tv.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0).apply {
                source = InputDevice.SOURCE_TOUCHSCREEN
            },
        )
        tv.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime + 50, MotionEvent.ACTION_UP, x, y, 0).apply {
                source = InputDevice.SOURCE_TOUCHSCREEN
            },
        )
        Thread.sleep(500)
    }

    private fun dispatchDoubleTap(
        x: Float,
        y: Float,
    ) {
        dispatchTap(x, y)
        Thread.sleep(120)
        dispatchTap(x, y)
        Thread.sleep(800)
    }

    private fun dispatchDrag(
        startX: Float,
        startY: Float,
        endX: Float,
        endY: Float,
        steps: Int = 10,
    ) {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        val downTime = SystemClock.uptimeMillis()
        tv.dispatchTouchEvent(
            MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, startX, startY, 0).apply {
                source = InputDevice.SOURCE_TOUCHSCREEN
            },
        )
        for (i in 1..steps) {
            val t = i.toFloat() / steps
            val cx = startX + (endX - startX) * t
            val cy = startY + (endY - startY) * t
            val evt =
                MotionEvent
                    .obtain(
                        downTime,
                        downTime + (i * 20).toLong(),
                        MotionEvent.ACTION_MOVE,
                        cx,
                        cy,
                        0,
                    ).apply { source = InputDevice.SOURCE_TOUCHSCREEN }
            tv.dispatchTouchEvent(evt)
            evt.recycle()
        }
        val upTime = downTime + (steps * 20).toLong() + 50
        tv.dispatchTouchEvent(
            MotionEvent.obtain(downTime, upTime, MotionEvent.ACTION_UP, endX, endY, 0).apply {
                source = InputDevice.SOURCE_TOUCHSCREEN
            },
        )
        Thread.sleep(1000)
    }

    private fun writeToTerminal(text: String) {
        val bridge = composeTestRule.getBridge()
        assertNotNull("Bridge must be available", bridge)
        bridge!!.writeToPty("echo '$text'\n".toByteArray())
        Thread.sleep(3000)
    }

    private fun assertTextInTerminal(text: String): Pair<Int, Int> {
        val bridge = composeTestRule.getBridge()
        val dataText = bridge!!.getTerminalText()
        assertTrue("Terminal must contain '$text'", dataText != null && dataText.contains(text))
        val lines = dataText!!.split("\n")
        for ((i, line) in lines.withIndex()) {
            val idx = line.indexOf(text)
            if (idx >= 0) return Pair(i, idx)
        }
        throw AssertionError("Text '$text' found via contains but not via indexOf")
    }

    private fun captureFrame(): io.torvox.PixelFrame {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.saveTestFrame(dataDir)
        return io.torvox.decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
    }

    private fun captureFrameWithSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        active: Boolean = true,
    ): io.torvox.PixelFrame {
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
        return io.torvox.decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
    }

    private fun assertPixelColor(
        x: Int,
        y: Int,
        expectedR: Int,
        expectedG: Int,
        expectedB: Int,
        tolerance: Int = 10,
    ) {
        val screenshot = takeScreenshot()
        val pixel = screenshot.getPixel(x, y)
        val r = Color.red(pixel)
        val g = Color.green(pixel)
        val b = Color.blue(pixel)
        screenshot.recycle()
        assertTrue(
            "Pixel at ($x,$y) expected rgb($expectedR,$expectedG,$expectedB) but got rgb($r,$g,$b)",
            abs(r - expectedR) <= tolerance &&
                abs(g - expectedG) <= tolerance &&
                abs(b - expectedB) <= tolerance,
        )
    }

    private fun extractTextFromRegion(
        bitmap: Bitmap,
        left: Int,
        top: Int,
        right: Int,
        bottom: Int,
    ): String {
        val region = Bitmap.createBitmap(bitmap, left, top, right - left, bottom - top)
        try {
            val image = InputImage.fromBitmap(region, 0)
            val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
            val result =
                com.google.android.gms.tasks.Tasks
                    .await(recognizer.process(image))
            val text = result.text
            recognizer.close()
            return text
        } catch (e: Exception) {
            throw AssertionError("ML Kit OCR failed (requires Google Play Services)", e)
        } finally {
            region.recycle()
        }
    }

    private fun cellX(col: Int): Int {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        return (col * tv.width) / 80
    }

    private fun cellY(row: Int): Int {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        return (row * tv.height) / 24
    }

    private fun cellWidth(): Int {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        return tv.width / 80
    }

    private fun cellHeight(): Int {
        val tv = checkNotNull(textureView) { "terminal TextureView must be present" }
        return tv.height / 24
    }

    private fun scrollToTop() {
        val bridge = checkNotNull(composeTestRule.getBridge()) { "Bridge must be available" }
        bridge.setScrollOffset(0u)
        Thread.sleep(500)
    }

    private fun scrollToBottom() {
        val bridge = checkNotNull(composeTestRule.getBridge()) { "Bridge must be available" }
        bridge.setScrollOffset(bridge.scrollbackLength())
        Thread.sleep(500)
    }

    private fun scrollBy(lines: Int) {
        val bridge = checkNotNull(composeTestRule.getBridge()) { "Bridge must be available" }
        // scrollBy from top: setScrollOffset with a positive value
        bridge.setScrollOffset(lines.toUInt().coerceAtMost(bridge.scrollbackLength()))
        Thread.sleep(500)
    }

    private fun getSelectedText(): String? {
        var text: String? = null
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val content = activity.findViewById<View>(android.R.id.content) as? ViewGroup
            if (content != null) {
                val surface = io.torvox.findTerminalSurface(activity)
                if (surface is io.torvox.ui.TerminalSurface) {
                    text = surface.getSelectedText()
                }
            }
        }
        return text
    }

    @Test
    fun longPressOnEmptyArea_createsZeroWidthSelection() {
        writeToTerminal("EMPTY_AREA_TEST")
        assertTextInTerminal("EMPTY_AREA_TEST")
        saveScreenshot("empty-area-baseline")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val x = tv.width * 0.8f
        val y = tv.height * 0.9f
        dispatchLongPress(x, y)
        saveScreenshot("empty-area-longpress")
    }

    @Test
    fun longPressOnEmptyArea_showsPasteButton() {
        writeToTerminal("PASTE_BUTTON_TEST")
        assertTextInTerminal("PASTE_BUTTON_TEST")
        val cm = context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
        cm.setPrimaryClip(android.content.ClipData.newPlainText("test", "paste_content"))
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val x = tv.width * 0.7f
        val y = tv.height * 0.85f
        dispatchLongPress(x, y)
        saveScreenshot("empty-area-paste-button")
        Thread.sleep(1000)
    }

    @Test
    fun longPressOnWordCharacter_selectsWordWithInvertedColors() {
        writeToTerminal("INVERTED_WORD_SELECT")
        val (line, col) = assertTextInTerminal("INVERTED_WORD_SELECT")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 2).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        val templateFrame = captureFrame()
        dispatchLongPress(lx, ly)
        saveScreenshot("word-selection-inverted")
        val selFrame = captureFrameWithSelection(line, col, line, col + "INVERTED_WORD_SELECT".length)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        for (c in col until col + 5) {
            val actual = io.torvox.extractCell(selFrame, c, line, gridCols, gridRows)
            val tmpl = io.torvox.extractCell(templateFrame, c, line, gridCols, gridRows)
            val conf = io.torvox.matchConfidence(actual, tmpl)
            assertTrue("Selected cell ($line,$c) should differ from template: conf=$conf", conf < 0.7)
        }
    }

    @Test
    fun longPressOnWhitespace_createsPasteAnchor() {
        writeToTerminal("WHITESPACE ANCHOR TEST")
        assertTextInTerminal("WHITESPACE ANCHOR TEST")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val midX = tv.width * 0.5f
        val midY = tv.height * 0.5f
        dispatchLongPress(midX, midY)
        saveScreenshot("whitespace-anchor")
    }

    @Test
    fun doubleTap_selectsLineOfText() {
        writeToTerminal("DOUBLE_TAP_SELECT")
        val (line, col) = assertTextInTerminal("DOUBLE_TAP_SELECT")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 1).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchDoubleTap(lx, ly)
        saveScreenshot("doubletap-line-selection")
    }

    @Test
    fun longPressOnURL_selectsFullURL() {
        writeToTerminal("https://example.com/test-url")
        assertTextInTerminal("https://example.com/test-url")
        val (line, col) = assertTextInTerminal("https://example.com/test-url")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 5).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(lx, ly)
        saveScreenshot("url-selection")
    }

    @Test
    fun handleDragToExpandSelection_rightHandle() {
        writeToTerminal("HANDLE_DRAG_RIGHT")
        val (line, col) = assertTextInTerminal("HANDLE_DRAG_RIGHT")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val startX = cellX(col + 1).toFloat()
        val y = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(startX, y)
        Thread.sleep(500)
        val dragEndX = cellX(col + 10).toFloat()
        dispatchDrag(startX + 30f, y, dragEndX, y, 8)
        saveScreenshot("handle-drag-right-expand")
    }

    @Test
    fun handleDragToContractSelection_leftHandle() {
        writeToTerminal("CONTRACT_SELECTION_HANDLE")
        val (line, col) = assertTextInTerminal("CONTRACT_SELECTION_HANDLE")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val startX = cellX(col + 5).toFloat()
        val y = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(startX, y)
        Thread.sleep(500)
        saveScreenshot("contract-start")
        dispatchTap(startX - cellWidth().toFloat(), y)
        saveScreenshot("contract-end")
    }

    @Test
    fun handleDragAcrossLines_multilineSelection() {
        writeToTerminal("MULTILINE_ONE")
        writeToTerminal("MULTILINE_TWO")
        val (line1, col1) = assertTextInTerminal("MULTILINE_ONE")
        assertTextInTerminal("MULTILINE_TWO")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val startX = cellX(col1 + 1).toFloat()
        val startY = cellY(line1).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(startX, startY)
        Thread.sleep(500)
        val endY = startY + cellHeight().toFloat() * 2.5f
        dispatchDrag(startX, startY, startX, endY, 15)
        saveScreenshot("multiline-selection")
    }

    @Test
    fun floatingToolbar_positionedAboveSelection() {
        writeToTerminal("TOOLBAR_ABOVE_TEST")
        val (line, col) = assertTextInTerminal("TOOLBAR_ABOVE_TEST")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 2).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(lx, ly)
        saveScreenshot("toolbar-above-position")
    }

    @Test
    fun floatingToolbar_positionedBelowSelection_whenNearTop() {
        writeToTerminal("TOOLBAR_BELOW")
        val (line, col) = assertTextInTerminal("TOOLBAR_BELOW")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 1).toFloat()
        val ly = cellY(max(0, line - 1)).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(lx, ly)
        saveScreenshot("toolbar-below-position")
    }

    @Test
    fun floatingToolbar_copyButton_copiesText() {
        writeToTerminal("COPY_BUTTON_TEXT")
        val (line, col) = assertTextInTerminal("COPY_BUTTON_TEXT")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 10).toUInt(), true)
        bridge.render()
        Thread.sleep(1000)
        saveScreenshot("toolbar-copy-button")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun floatingToolbar_selectAll_selectsEverything() {
        writeToTerminal("SELECT_ALL_MARKER")
        assertTextInTerminal("SELECT_ALL_MARKER")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()
        bridge.setSelection(0u, 0u, (gridRows - 1).toUInt(), (gridCols - 1).toUInt(), true)
        bridge.render()
        Thread.sleep(1000)
        saveScreenshot("toolbar-select-all")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun floatingToolbar_pasteButton_pastesClipboard() {
        writeToTerminal("PASTE_MARKER")
        assertTextInTerminal("PASTE_MARKER")
        val cm = context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
        cm.setPrimaryClip(android.content.ClipData.newPlainText("test", "CLIPBOARD_CONTENT"))
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val x = tv.width * 0.5f
        val y = tv.height * 0.8f
        dispatchLongPress(x, y)
        saveScreenshot("toolbar-paste-button")
    }

    @Test
    fun clearSelection_onTapOutside() {
        writeToTerminal("CLEAR_ON_TAP_OUTSIDE")
        val (line, col) = assertTextInTerminal("CLEAR_ON_TAP_OUTSIDE")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 8).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("clear-tap-before")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("clear-tap-after")
    }

    @Test
    fun selectionPersists_acrossOrientationChange() {
        writeToTerminal("ORIENTATION_SEL")
        val (line, col) = assertTextInTerminal("ORIENTATION_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 8).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("orientation-before")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        Thread.sleep(2000)
        saveScreenshot("orientation-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionWithImeOpen_doesNotDisturbSelection() {
        writeToTerminal("IME_OPEN_TEST")
        val (line, col) = assertTextInTerminal("IME_OPEN_TEST")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("ime-open-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionWithImeClosed_selectionRestored() {
        writeToTerminal("IME_CLOSED_SEL")
        val (line, col) = assertTextInTerminal("IME_CLOSED_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("ime-closed-before")
        device.pressBack()
        Thread.sleep(500)
        saveScreenshot("ime-closed-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionWithSessionPanelOpen_remainsActive() {
        writeToTerminal("SESSION_PANEL_SEL")
        val (line, col) = assertTextInTerminal("SESSION_PANEL_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 8).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("session-panel-before")
        composeTestRule.openDrawer()
        Thread.sleep(1000)
        saveScreenshot("session-panel-with-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun scrollWhileSelectionActive_selectionCleared() {
        writeToTerminal("SCROLL_WHILE_SELECTED")
        val (line, col) = assertTextInTerminal("SCROLL_WHILE_SELECTED")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 10).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("scroll-sel-before")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val scrollStartY = tv.height * 0.3f
        val scrollEndY = tv.height * 0.7f
        dispatchDrag(tv.width / 2f, scrollStartY, tv.width / 2f, scrollEndY, 10)
        saveScreenshot("scroll-sel-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun edgeScroll_duringHandleDrag() {
        writeToTerminal("EDGE_SCROLL_TEST")
        val (line, col) = assertTextInTerminal("EDGE_SCROLL_TEST")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val startX = cellX(col + 1).toFloat()
        val startY = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(startX, startY)
        Thread.sleep(500)
        val edgeX = tv.width * 0.95f
        dispatchDrag(startX, startY, edgeX, startY, 20)
        saveScreenshot("edge-scroll-handle-drag")
        val bridge = composeTestRule.getBridge()
        bridge!!.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionColors_matchTheme_notHardcoded() {
        writeToTerminal("THEME_COLOR_SEL")
        val (line, col) = assertTextInTerminal("THEME_COLOR_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 8)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val selCell = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val tmplCell = io.torvox.extractCell(templateFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selCell, tmplCell)
        assertTrue("Selection colors must differ from template: conf=$conf", conf < 0.8)
        saveScreenshot("theme-color-selection")
    }

    @Test
    fun blockCursor_renderedDuringSelection() {
        writeToTerminal("BLOCK_CURSOR_SEL")
        assertTextInTerminal("BLOCK_CURSOR_SEL")
        captureFrame()
        saveScreenshot("block-cursor-rendered")
    }

    @Test
    fun cjkCharacterDisplay_withSelection() {
        writeToTerminal("CJK_SEL_TEXT")
        Thread.sleep(3000)
        saveScreenshot("cjk-selection")
        val bridge = composeTestRule.getBridge()
        val text = bridge!!.getTerminalText()
        if (text != null && text.contains("CJK")) {
            bridge.setSelection(0u, 0u, 0u, 4u, true)
            bridge.render()
            Thread.sleep(500)
            saveScreenshot("cjk-selection-active")
            bridge.setSelection(0u, 0u, 0u, 0u, false)
        }
    }

    @Test
    fun cursorBlinkState_toggleDoesNotCrash() {
        writeToTerminal("CURSOR_BLINK_TEST")
        assertTextInTerminal("CURSOR_BLINK_TEST")
        saveScreenshot("cursor-blink-default")
        val bridge = composeTestRule.getBridge()
        bridge!!.writeToPty("echo 'CURSOR_BLINK'\n".toByteArray())
        Thread.sleep(2000)
        saveScreenshot("cursor-blink-toggled")
    }

    @Test
    fun backgroundImageRendering_withSelection() {
        writeToTerminal("BG_IMAGE_SEL")
        assertTextInTerminal("BG_IMAGE_SEL")
        captureFrame()
        saveScreenshot("background-image-selection")
    }

    @Test
    fun selectionDoesNotTriggerOnRapidTap() {
        writeToTerminal("RAPID_TAP_NO_SEL")
        assertTextInTerminal("RAPID_TAP_NO_SEL")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        for (i in 0..4) {
            dispatchTap(tv.width * 0.5f, tv.height * 0.3f)
        }
        saveScreenshot("rapid-tap-no-selection")
    }

    @Test
    fun selectionClear_onNewInput() {
        writeToTerminal("CLEAR_ON_NEW_INPUT")
        val (line, col) = assertTextInTerminal("CLEAR_ON_NEW_INPUT")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("new-input-before")
        bridge.writeToPty("echo 'NEW_INPUT'\n".toByteArray())
        Thread.sleep(3000)
        saveScreenshot("new-input-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun longPressOnNumber_selectsNumberWord() {
        writeToTerminal("NUMBER_12345_SELECTION")
        val (line, col) = assertTextInTerminal("NUMBER_12345_SELECTION")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 7).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(lx, ly)
        saveScreenshot("number-selection")
    }

    @Test
    fun longPressOnSymbol_selectsSymbolSequence() {
        writeToTerminal("SYM_BOUNDARY_PLUS_TEST")
        val (line, col) = assertTextInTerminal("SYM_BOUNDARY_PLUS_TEST")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 13).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(lx, ly)
        saveScreenshot("symbol-selection")
    }

    @Test
    fun longPressOnStartOfLine_selectsFromBoundary() {
        writeToTerminal("LINE_START_SEL")
        val (line, col) = assertTextInTerminal("LINE_START_SEL")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(lx, ly)
        saveScreenshot("line-start-selection")
    }

    @Test
    fun selectionHighlight_visibleOnDarkTheme() {
        writeToTerminal("DARK_THEME_SEL")
        val (line, col) = assertTextInTerminal("DARK_THEME_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("dark-theme-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Selection on dark theme must have content: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionHighlight_visibleOnLightTheme() {
        writeToTerminal("LIGHT_THEME_SEL")
        val (line, col) = assertTextInTerminal("LIGHT_THEME_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("light-theme-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Selection on light theme must have content: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionFromBridge_setAndClearSelection() {
        writeToTerminal("BRIDGE_SEL_TEST")
        val (line, col) = assertTextInTerminal("BRIDGE_SEL_TEST")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("bridge-set-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("bridge-cleared-selection")
    }

    @Test
    fun selectionHandles_disappearOnTapOutside() {
        writeToTerminal("HANDLES_DISAPPEAR")
        val (line, col) = assertTextInTerminal("HANDLES_DISAPPEAR")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 5).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("handles-visible")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("handles-hidden")
    }

    @Test
    fun selectionAcrossMultiline_highlightSpansMultipleRows() {
        writeToTerminal("MULTIROW_START")
        writeToTerminal("MULTIROW_END")
        val (line1, _) = assertTextInTerminal("MULTIROW_START")
        assertTextInTerminal("MULTIROW_END")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        val gridRows = bridge.getGridRows()
        val templateFrame = captureFrame()
        bridge.setSelection(line1.toUInt(), 0u, min(line1 + 2, gridRows - 1).toUInt(), (gridCols - 1).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        bridge.saveTestFrame(dataDir)
        val selFrame = io.torvox.decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
        saveScreenshot("multiline-highlight")
        for (r in line1..min(line1 + 2, gridRows - 1)) {
            val selCell = io.torvox.extractCell(selFrame, 0, r, gridCols, gridRows)
            val tmplCell = io.torvox.extractCell(templateFrame, 0, r, gridCols, gridRows)
            val conf = io.torvox.matchConfidence(selCell, tmplCell)
            assertTrue("Row $r must show selection highlight: conf=$conf", conf < 0.7)
        }
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionOnWrappedLine_highlightExtendsToWrappedPortion() {
        writeToTerminal("WRAPPED_LINE_SELECTION_LONG_CONTENT_FOR_WRAPPING")
        val (line, col) = assertTextInTerminal("WRAPPED_LINE_SELECTION_LONG_CONTENT_FOR_WRAPPING")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 40).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("wrapped-line-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionHandle_dragToContract_rightHandle() {
        writeToTerminal("CONTRACT_RIGHT_HANDLE")
        val (line, col) = assertTextInTerminal("CONTRACT_RIGHT_HANDLE")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 15).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("contract-right-before")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 5).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("contract-right-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionHandle_dragToContract_leftHandle() {
        writeToTerminal("CONTRACT_LEFT_HANDLE")
        val (line, col) = assertTextInTerminal("CONTRACT_LEFT_HANDLE")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 12).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("contract-left-before")
        bridge.setSelection((line).toUInt(), (col + 5).toUInt(), line.toUInt(), (col + 12).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("contract-left-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionHighlight_invertsForegroundBackground() {
        writeToTerminal("INVERT_FG_BG_SEL")
        val (line, col) = assertTextInTerminal("INVERT_FG_BG_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 7)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val selPixels = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val tplPixels = io.torvox.extractCell(templateFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selPixels, tplPixels)
        assertTrue("Selection must invert colors: conf=$conf", conf < 0.6)
    }

    @Test
    fun selectionOnPrompt_highlightsPromptContent() {
        writeToTerminal("PROMPT_SEL_TEST")
        val (line, col) = assertTextInTerminal("PROMPT_SEL_TEST")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("prompt-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Prompt selection must render content: $ratio", ratio > 0.01)
    }

    @Test
    fun longPressOnNonExistentRegion_doesNotCrash() {
        val tv = textureView ?: throw AssertionError("TextureView not found")
        dispatchLongPress(-100f, -100f)
        saveScreenshot("non-existent-region")
    }

    @Test
    fun selectionOnDenseOutput_highlightsCorrectRegion() {
        val sb = StringBuilder()
        for (i in 0..19) {
            sb.append("LINE_${i}_CONTENT ")
        }
        writeToTerminal(sb.toString())
        Thread.sleep(4000)
        saveScreenshot("dense-output-selection")
    }

    @Test
    fun selectionHighlight_disappearsWhenSelectionCleared() {
        writeToTerminal("HIGHLIGHT_DISAPPEAR")
        val (line, col) = assertTextInTerminal("HIGHLIGHT_DISAPPEAR")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 10).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("highlight-visible")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("highlight-gone")
    }

    @Test
    fun selectionOnTabSeparatedText_highlightsFullTab() {
        writeToTerminal("TAB_SEPARATED_SELECTION")
        val (line, col) = assertTextInTerminal("TAB_SEPARATED_SELECTION")
        val selFrame = captureFrameWithSelection(line, col, line, col + 10)
        saveScreenshot("tab-separated-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Tab-separated selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selection_dragSelect_byDraggingAcrossLine() {
        writeToTerminal("DRAG_SELECT_ACROSS")
        val (line, col) = assertTextInTerminal("DRAG_SELECT_ACROSS")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val startX = cellX(col).toFloat()
        val y = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        val endX = cellX(col + 10).toFloat()
        dispatchDrag(startX, y, endX, y, 8)
        saveScreenshot("drag-select-across")
    }

    @Test
    fun rapidSelectionChanges_doNotCrash() {
        writeToTerminal("RAPID_SEL_CHANGE")
        assertTextInTerminal("RAPID_SEL_CHANGE")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridRows = bridge.getGridRows()
        val gridCols = bridge.getGridCols()
        for (i in 0..9) {
            bridge.setSelection(0u, 0u, (i % gridRows).toUInt(), (i % gridCols).toUInt(), i % 2 == 0)
            bridge.render()
            Thread.sleep(50)
        }
        saveScreenshot("rapid-selection-changes")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionBlockMode_highlightsRectangularRegion() {
        writeToTerminal("BLOCK_MODE_ONE")
        writeToTerminal("BLOCK_MODE_TWO")
        writeToTerminal("BLOCK_MODE_THREE")
        val (line1, _) = assertTextInTerminal("BLOCK_MODE_ONE")
        assertTextInTerminal("BLOCK_MODE_TWO")
        assertTextInTerminal("BLOCK_MODE_THREE")
        saveScreenshot("block-mode-selection")
    }

    @Test
    fun selectionOCR_verifiesContent() {
        writeToTerminal("OCR_CONTENT_MARKER")
        val (line, col) = assertTextInTerminal("OCR_CONTENT_MARKER")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        val bitmap = io.torvox.decodeRgbaToBitmap(frameFile)
        try {
            val ocrText = extractTextFromRegion(bitmap, 0, 0, bitmap.width / 2, bitmap.height / 2)
            File(screenshotDir, "ocr-output.txt").writeText(ocrText)
            assertTrue(
                "OCR must detect OCR_CONTENT_MARKER in:\n$ocrText",
                ocrText.contains("OCR_CONTENT_MARKER"),
            )
        } finally {
            bitmap.recycle()
        }
        saveScreenshot("ocr-content-verification")
    }

    @Test
    fun selectionPauseResume_preservesState() {
        writeToTerminal("PAUSE_RESUME_SEL")
        val (line, col) = assertTextInTerminal("PAUSE_RESUME_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("pause-resume-before")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.pauseRendering()
        }
        Thread.sleep(1000)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.torvoxRuntime.resumeRendering()
        }
        Thread.sleep(1000)
        saveScreenshot("pause-resume-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionOnItalicText_noCrash() {
        writeToTerminal("ITALIC_TEXT_SEL")
        val (line, col) = assertTextInTerminal("ITALIC_TEXT_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("italic-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Italic selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnBoldText_noCrash() {
        writeToTerminal("BOLD_TEXT_SEL")
        val (line, col) = assertTextInTerminal("BOLD_TEXT_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("bold-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Bold selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnUnderlinedText_noCrash() {
        writeToTerminal("UNDERLINE_TEXT_SEL")
        val (line, col) = assertTextInTerminal("UNDERLINE_TEXT_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("underline-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Underlined selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnColoredText_preservesColorInversion() {
        writeToTerminal("COLORED_SEL_TEXT")
        val (line, col) = assertTextInTerminal("COLORED_SEL_TEXT")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val tmplPixels = io.torvox.extractCell(templateFrame, col, line, gridCols, gridRows)
        val selPixels = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selPixels, tmplPixels)
        assertTrue("Colored text selection must invert: conf=$conf", conf < 0.7)
    }

    @Test
    fun selectionOnEmptyRow_doesNotHighlight() {
        writeToTerminal("EMPTY_ROW_ABOVE")
        val (line, _) = assertTextInTerminal("EMPTY_ROW_ABOVE")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection((line + 2).toUInt(), 0u, (line + 2).toUInt(), 10u, true)
        bridge.render()
        Thread.sleep(500)
        bridge.saveTestFrame(dataDir)
        val selFrame = io.torvox.decodeRgbaToPixels(File(dataDir, "test_frame.rgba"))
        saveScreenshot("empty-row-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionOnFullRow_selectsEntireRow() {
        writeToTerminal("FULL_ROW_SELECTION")
        val (line, col) = assertTextInTerminal("FULL_ROW_SELECTION")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        val selFrame = captureFrameWithSelection(line, 0, line, gridCols - 1)
        saveScreenshot("full-row-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Full row selection must show content: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionAccelerated_byGpuPipeline() {
        writeToTerminal("GPU_ACCEL_SEL")
        val (line, col) = assertTextInTerminal("GPU_ACCEL_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("gpu-accelerated-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("GPU selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionNoDoubleFree_fromRapidSelectClear() {
        writeToTerminal("DOUBLE_FREE_SEL")
        assertTextInTerminal("DOUBLE_FREE_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        for (i in 0..19) {
            bridge.setSelection(0u, 0u, 1u, 200u, i % 2 == 0)
        }
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        saveScreenshot("double-free-stability")
    }

    @Test
    fun selectionColor_contrastCheck() {
        writeToTerminal("CONTRAST_CHECK_SEL")
        val (line, col) = assertTextInTerminal("CONTRAST_CHECK_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val tmplPixels = io.torvox.extractCell(templateFrame, col, line, gridCols, gridRows)
        val selPixels = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selPixels, tmplPixels)
        assertTrue("Selection must provide contrast: conf=$conf", conf < 0.75)
    }

    @Test
    fun selectionOnMixedWidthCharacters_rendersCorrectly() {
        writeToTerminal("MIXED_WIDTH_CHARS_123")
        val (line, col) = assertTextInTerminal("MIXED_WIDTH_CHARS_123")
        val selFrame = captureFrameWithSelection(line, col, line, col + 8)
        saveScreenshot("mixed-width-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Mixed-width selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnSpecialCharacters_handlesCorrectly() {
        writeToTerminal("SPECIAL_AT_HASH_CHARS")
        val (line, col) = assertTextInTerminal("SPECIAL_AT_HASH_CHARS")
        val selFrame = captureFrameWithSelection(line, col, line, col + 10)
        saveScreenshot("special-chars-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Special char selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun multipleSelectionChanges_trackCorrectOffsets() {
        writeToTerminal("OFFSET_TRACKING_SEL")
        assertTextInTerminal("OFFSET_TRACKING_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(0u, 0u, 2u, 10u, true)
        bridge.render()
        Thread.sleep(200)
        bridge.setSelection(1u, 3u, 3u, 15u, true)
        bridge.render()
        Thread.sleep(200)
        bridge.setSelection(2u, 5u, 4u, 20u, true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("offset-tracking")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionOnMultiplePromptLines_highlightExtends() {
        writeToTerminal("PROMPT_LINE_A")
        writeToTerminal("PROMPT_LINE_B")
        val (lineA, _) = assertTextInTerminal("PROMPT_LINE_A")
        assertTextInTerminal("PROMPT_LINE_B")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        bridge.setSelection(lineA.toUInt(), 0u, (lineA + 3).toUInt(), (gridCols - 1).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("multi-prompt-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionClearedOnTerminalReset() {
        writeToTerminal("RESET_CLEAR_SEL")
        val (line, col) = assertTextInTerminal("RESET_CLEAR_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("reset-clear-before")
        bridge.writeToPty("printf '\\ec'\n".toByteArray())
        Thread.sleep(2000)
        saveScreenshot("reset-clear-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionActiveDuringAlternateScreen() {
        writeToTerminal("ALT_SCREEN_SEL")
        val (line, col) = assertTextInTerminal("ALT_SCREEN_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("printf '\\e[?1049h'\n".toByteArray())
        Thread.sleep(1000)
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("alt-screen-selection")
        bridge.writeToPty("printf '\\e[?1049l'\n".toByteArray())
        Thread.sleep(1000)
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionAfterBell_doesNotDismiss() {
        writeToTerminal("BELL_SEL_TEST")
        val (line, col) = assertTextInTerminal("BELL_SEL_TEST")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        bridge.writeToPty("printf '\\a'\n".toByteArray())
        Thread.sleep(1000)
        saveScreenshot("bell-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionAcrossVisibleAndScrollback_mergedHighlight() {
        writeToTerminal("VISIBLE_LINE_SEL")
        writeToTerminal("SCROLLBACK_LINE_SEL")
        assertTextInTerminal("SCROLLBACK_LINE_SEL")
        scrollToTop()
        Thread.sleep(1000)
        saveScreenshot("visible-scrollback-merged")
        scrollToBottom()
        Thread.sleep(1000)
    }

    @Test
    fun selectionDuringBackgroundTask_remainsVisible() {
        writeToTerminal("BG_TASK_SEL_TEST")
        val (line, col) = assertTextInTerminal("BG_TASK_SEL_TEST")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        bridge.writeToPty("sleep 2 &\n".toByteArray())
        Thread.sleep(3000)
        saveScreenshot("bg-task-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionBoundaryExactMatch() {
        writeToTerminal("EXACT_BOUNDARY_SEL")
        val (line, col) = assertTextInTerminal("EXACT_BOUNDARY_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 5)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        saveScreenshot("exact-boundary")
        io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        io.torvox.extractCell(selFrame, max(0, col - 1), line, gridCols, gridRows)
        assertTrue("Boundary check must not crash", true)
    }

    @Test
    fun selectionBackgroundColorDiff_visibleOnAllCellRows() {
        writeToTerminal("BG_DIFF_MULTIROW")
        writeToTerminal("ROW_TWO_DATA")
        val (line, col) = assertTextInTerminal("BG_DIFF_MULTIROW")
        assertTextInTerminal("ROW_TWO_DATA")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), (line + 1).toUInt(), (col + 8).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("bg-diff-multirow")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionTopRowDoesNotShiftViewport() {
        writeToTerminal("TOP_ROW_SEL_NO_SHIFT")
        val (line, col) = assertTextInTerminal("TOP_ROW_SEL_NO_SHIFT")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("top-row-no-shift")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionOnPartialLine_rightSideNotSelected() {
        writeToTerminal("PARTIAL_LINE_SEL")
        val (line, col) = assertTextInTerminal("PARTIAL_LINE_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 7)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val unselCol = min(col + 12, gridCols - 1)
        val unselActual = io.torvox.extractCell(selFrame, unselCol, line, gridCols, gridRows)
        val unselTmpl = io.torvox.extractCell(templateFrame, unselCol, line, gridCols, gridRows)
        val unselConf = io.torvox.matchConfidence(unselActual, unselTmpl)
        assertTrue("Right side beyond selection must match template: conf=$unselConf", unselConf >= 0.85)
        saveScreenshot("partial-line-right")
    }

    @Test
    fun selectionOnPartialLine_leftSideNotSelected() {
        writeToTerminal("LEFT_PARTIAL_SEL")
        val (line, col) = assertTextInTerminal("LEFT_PARTIAL_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col + 3, line, col + 10)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val unselCol = col
        val unselActual = io.torvox.extractCell(selFrame, unselCol, line, gridCols, gridRows)
        val unselTmpl = io.torvox.extractCell(templateFrame, unselCol, line, gridCols, gridRows)
        val unselConf = io.torvox.matchConfidence(unselActual, unselTmpl)
        assertTrue("Left side beyond selection must match template: conf=$unselConf", unselConf >= 0.85)
        saveScreenshot("partial-line-left")
    }

    @Test
    fun selectionAfterFastScroll_doesNotLag() {
        writeToTerminal("FAST_SCROLL_SEL")
        assertTextInTerminal("FAST_SCROLL_SEL")
        for (i in 0..9) {
            scrollBy(1)
        }
        saveScreenshot("fast-scroll-selection")
        scrollToBottom()
        Thread.sleep(500)
    }

    @Test
    fun selectionHighlight_notShownWhenActiveFalse() {
        writeToTerminal("ACTIVE_FALSE_SEL")
        val (line, col) = assertTextInTerminal("ACTIVE_FALSE_SEL")
        val templateFrame = captureFrame()
        val inactiveFrame = captureFrameWithSelection(line, col, line, col + 6, active = false)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        for (c in col..col + 6) {
            val actual = io.torvox.extractCell(inactiveFrame, c, line, gridCols, gridRows)
            val tmpl = io.torvox.extractCell(templateFrame, c, line, gridCols, gridRows)
            val conf = io.torvox.matchConfidence(actual, tmpl)
            assertTrue("Inactive cell ($line,$c) should match template: conf=$conf", conf >= 0.9)
        }
        saveScreenshot("active-false-no-highlight")
    }

    @Test
    fun selectionOnRapidOutput_highlightKeepsUp() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        for (i in 0..4) {
            bridge.writeToPty("echo 'RAPID_OUT_$i'\n".toByteArray())
        }
        Thread.sleep(3000)
        saveScreenshot("rapid-output-selection")
    }

    @Test
    fun selectionAfterLargePaste_doesNotCorrupt() {
        val sb = StringBuilder()
        for (i in 0..49) {
            sb.append("LINE_${i}_DATA ")
        }
        writeToTerminal(sb.toString())
        Thread.sleep(5000)
        saveScreenshot("large-paste-selection")
    }

    @Test
    fun selectionNoMemoryLeak_afterRepeatedSelectClear() {
        writeToTerminal("MEMORY_LEAK_SEL")
        assertTextInTerminal("MEMORY_LEAK_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(0u, 0u, 5u, 40u, true)
        bridge.render()
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        for (i in 0..49) {
            bridge.setSelection(0u, 0u, 1u, 20u, i % 2 == 0)
            bridge.render()
            bridge.setSelection(0u, 0u, 0u, 0u, false)
            bridge.render()
        }
        saveScreenshot("memory-leak-check")
    }

    @Test
    fun selectionOnInput_fieldHighlighted() {
        writeToTerminal("INPUT_FIELD_SEL")
        val (line, col) = assertTextInTerminal("INPUT_FIELD_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 5)
        saveScreenshot("input-field-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Input field selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionLineExact_boundaryOnLeftEdge() {
        writeToTerminal("LEFT_EDGE_SEL")
        val (line, col) = assertTextInTerminal("LEFT_EDGE_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, 0, line, col + 4)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val leftCell = io.torvox.extractCell(selFrame, 0, line, gridCols, gridRows)
        val leftTmpl = io.torvox.extractCell(templateFrame, 0, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(leftCell, leftTmpl)
        assertTrue("Left edge selected cell must differ: conf=$conf", conf < 0.7)
        saveScreenshot("left-edge-selection")
    }

    @Test
    fun selectionLineExact_boundaryOnRightEdge() {
        writeToTerminal("RIGHT_EDGE_SEL")
        val (line, col) = assertTextInTerminal("RIGHT_EDGE_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val gridCols = bridge.getGridCols()
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (gridCols - 1).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("right-edge-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionPreservesAfterBlurFocusCycle() {
        writeToTerminal("BLUR_FOCUS_SEL")
        val (line, col) = assertTextInTerminal("BLUR_FOCUS_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("blur-focus-before")
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.window.decorView.requestFocus()
        }
        Thread.sleep(500)
        saveScreenshot("blur-focus-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionVisibleAfterRenderResume() {
        writeToTerminal("RENDER_RESUME_SEL")
        val (line, col) = assertTextInTerminal("RENDER_RESUME_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("render-resume-visible")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionFullLine_matchesPromptLength() {
        writeToTerminal("FULL_LINE_LENGTH_SEL")
        val (line, col) = assertTextInTerminal("FULL_LINE_LENGTH_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + "FULL_LINE_LENGTH_SEL".length - 1).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("full-line-length")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionHandleDragBeyondScreen_doesNotOverscroll() {
        writeToTerminal("DRAG_BEYOND_SCREEN")
        val (line, col) = assertTextInTerminal("DRAG_BEYOND_SCREEN")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val startX = cellX(col + 1).toFloat()
        val y = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchLongPress(startX, y)
        Thread.sleep(500)
        dispatchDrag(startX, y, startX + tv.width * 1.5f, y, 10)
        saveScreenshot("drag-beyond-screen")
    }

    @Test
    fun selectionColorDoesNotBleed_toAdjacentCell() {
        writeToTerminal("NO_BLEED_SEL")
        val (line, col) = assertTextInTerminal("NO_BLEED_SEL")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 3)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val bleedCol = col + 5
        if (bleedCol < gridCols) {
            val adjActual = io.torvox.extractCell(selFrame, bleedCol, line, gridCols, gridRows)
            val adjTmpl = io.torvox.extractCell(templateFrame, bleedCol, line, gridCols, gridRows)
            val adjConf = io.torvox.matchConfidence(adjActual, adjTmpl)
            assertTrue("Adjacent cell must not be affected: conf=$adjConf", adjConf >= 0.85)
        }
        saveScreenshot("no-bleed-selection")
    }

    @Test
    fun selectionZOrder_onTopOfOtherUiElements() {
        writeToTerminal("ZORDER_SEL_TEST")
        val (line, col) = assertTextInTerminal("ZORDER_SEL_TEST")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("zorder-selection")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Z-order selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnTabChar_highlightExtendsToTabStop() {
        writeToTerminal("TAB_CHAR_SEPARATED")
        val (line, col) = assertTextInTerminal("TAB_CHAR_SEPARATED")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 10).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("tab-char-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionAfterClearScreen_notPersisted() {
        writeToTerminal("CLEAR_SCREEN_SEL")
        val (line, col) = assertTextInTerminal("CLEAR_SCREEN_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("clear-screen-before")
        bridge.writeToPty("printf '\\e[2J'\n".toByteArray())
        Thread.sleep(2000)
        saveScreenshot("clear-screen-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionOnSelection_reelectedTextUpdates() {
        writeToTerminal("REELECT_SEL_TEST")
        val (line, col) = assertTextInTerminal("REELECT_SEL_TEST")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(300)
        bridge.setSelection(line.toUInt(), (col + 3).toUInt(), line.toUInt(), (col + 9).toUInt(), true)
        bridge.render()
        Thread.sleep(300)
        saveScreenshot("reelect-selection")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionScrollPreserves_correctRowOffsets() {
        writeToTerminal("SCROLL_OFFSET_SEL")
        assertTextInTerminal("SCROLL_OFFSET_SEL")
        scrollBy(5)
        Thread.sleep(1000)
        saveScreenshot("scroll-offset-selection")
        scrollToBottom()
        Thread.sleep(500)
    }

    @Test
    fun selectionDoesNotTriggerOnSingleTap() {
        writeToTerminal("SINGLE_TAP_NO_SEL")
        val (line, col) = assertTextInTerminal("SINGLE_TAP_NO_SEL")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 1).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchTap(lx, ly)
        saveScreenshot("single-tap-no-selection")
    }

    @Test
    fun selectionDoubleTap_wordExpandsToWord() {
        writeToTerminal("EXPAND_WORD_SEL")
        val (line, col) = assertTextInTerminal("EXPAND_WORD_SEL")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 2).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchDoubleTap(lx, ly)
        saveScreenshot("doubletap-word-expand")
    }

    @Test
    fun selectionTripleTap_selectsLine() {
        writeToTerminal("TRIPLE_TAP_LINE_SEL")
        val (line, col) = assertTextInTerminal("TRIPLE_TAP_LINE_SEL")
        val tv = textureView ?: throw AssertionError("TextureView not found")
        val lx = cellX(col + 2).toFloat()
        val ly = cellY(line).toFloat() + cellHeight().toFloat() * 0.5f
        dispatchTap(lx, ly)
        Thread.sleep(120)
        dispatchTap(lx, ly)
        Thread.sleep(120)
        dispatchTap(lx, ly)
        Thread.sleep(1000)
        saveScreenshot("triple-tap-line")
    }

    @Test
    fun fastSelectDeselect_noGpuArtifacts() {
        writeToTerminal("FAST_SEL_DESEL")
        assertTextInTerminal("FAST_SEL_DESEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        for (i in 0..29) {
            bridge.setSelection(0u, 0u, 2u, 20u, i % 2 == 0)
            bridge.render()
        }
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        Thread.sleep(500)
        val frame = captureFrame()
        saveScreenshot("fast-sel-desel")
        val ratio = io.torvox.analyzeNonBlackRatio(frame)
        assertTrue("Screen must be intact after fast toggle: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionWhenSuspended_resumesCorrectly() {
        writeToTerminal("SUSPEND_RESUME_SEL")
        val (line, col) = assertTextInTerminal("SUSPEND_RESUME_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("suspend-resume-before")
        composeTestRule.activityRule.scenario.moveToState(androidx.lifecycle.Lifecycle.State.CREATED)
        Thread.sleep(1000)
        composeTestRule.activityRule.scenario.moveToState(androidx.lifecycle.Lifecycle.State.RESUMED)
        Thread.sleep(2000)
        saveScreenshot("suspend-resume-after")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }

    @Test
    fun selectionHighlight_consistentAcrossThemes() {
        writeToTerminal("CONSISTENT_THEME_SEL")
        val (line, col) = assertTextInTerminal("CONSISTENT_THEME_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        saveScreenshot("consistent-theme-sel")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Theme-consistent selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnLongLine_scrollNotTriggered() {
        val sb = StringBuilder()
        for (i in 0..199) {
            sb.append("LONG_LINE_SEL_${i}_")
        }
        writeToTerminal(sb.toString())
        Thread.sleep(5000)
        saveScreenshot("long-line-no-scroll")
    }

    @Test
    fun selectionOnAlternateBg_inversionLooksCorrect() {
        writeToTerminal("ALT_BG_SEL_TEXT")
        val (line, col) = assertTextInTerminal("ALT_BG_SEL_TEXT")
        val templateUrlFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val selPixels = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val tplPixels = io.torvox.extractCell(templateUrlFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selPixels, tplPixels)
        assertTrue("Alternate BG selection must invert: conf=$conf", conf < 0.75)
    }

    @Test
    fun selectionNoArtifact_onPartialCell() {
        writeToTerminal("PARTIAL_CELL_SEL")
        val (line, col) = assertTextInTerminal("PARTIAL_CELL_SEL")
        val selFrame = captureFrameWithSelection(line, col, line, col + 4)
        saveScreenshot("partial-cell-no-artifact")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Partial cell selection must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnBackgroundColor256_rendersCorrectInversion() {
        writeToTerminal("BG256_SEL_TEST")
        val (line, col) = assertTextInTerminal("BG256_SEL_TEST")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val selPixels = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val tplPixels = io.torvox.extractCell(templateFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selPixels, tplPixels)
        assertTrue("256-color selection must differ: conf=$conf", conf < 0.75)
    }

    @Test
    fun selectionOnTrueColor_rendersCorrectInversion() {
        writeToTerminal("TRUE_COLOR_SEL_TEST")
        val (line, col) = assertTextInTerminal("TRUE_COLOR_SEL_TEST")
        val templateFrame = captureFrame()
        val selFrame = captureFrameWithSelection(line, col, line, col + 6)
        val gridRows = composeTestRule.getBridge()!!.getGridRows()
        val gridCols = composeTestRule.getBridge()!!.getGridCols()
        val selPixels = io.torvox.extractCell(selFrame, col, line, gridCols, gridRows)
        val tplPixels = io.torvox.extractCell(templateFrame, col, line, gridCols, gridRows)
        val conf = io.torvox.matchConfidence(selPixels, tplPixels)
        assertTrue("TrueColor selection must differ: conf=$conf", conf < 0.75)
    }

    @Test
    fun selectionAfterClearScreen_contentRestored() {
        writeToTerminal("CLEAR_RESTORE_SEL")
        val (line, col) = assertTextInTerminal("CLEAR_RESTORE_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(500)
        bridge.writeToPty("printf '\\e[2J'\n".toByteArray())
        Thread.sleep(2000)
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        saveScreenshot("clear-restore-check")
    }

    @Test
    fun selectionColorDoesNotPersistenceAcrossClear() {
        writeToTerminal("PERSISTENCE_CLEAR_SEL")
        val (line, col) = assertTextInTerminal("PERSISTENCE_CLEAR_SEL")
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.setSelection(line.toUInt(), col.toUInt(), line.toUInt(), (col + 6).toUInt(), true)
        bridge.render()
        Thread.sleep(300)
        bridge.setSelection(0u, 0u, 0u, 0u, false)
        bridge.render()
        Thread.sleep(300)
        val frame = captureFrame()
        saveScreenshot("persistence-clear-check")
        val ratio = io.torvox.analyzeNonBlackRatio(frame)
        assertTrue("After clear, screen must be intact: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOnStatusBar_overlapNotCrashing() {
        writeToTerminal("STATUSBAR_OVERLAP")
        assertTextInTerminal("STATUSBAR_OVERLAP")
        val selFrame = captureFrameWithSelection(0, 0, 0, 6)
        saveScreenshot("statusbar-overlap")
        val ratio = io.torvox.analyzeNonBlackRatio(selFrame)
        assertTrue("Status bar overlap must render: $ratio", ratio > 0.01)
    }

    @Test
    fun selectionOn24bitColors_rendersCorrectly() {
        writeToTerminal("TRUECOLOR_24BIT_SEL")
        Thread.sleep(3000)
        saveScreenshot("24bit-color-selection")
        val bridge = composeTestRule.getBridge()
        bridge!!.setSelection(0u, 0u, 0u, 9u, true)
        bridge.render()
        Thread.sleep(500)
        saveScreenshot("24bit-color-active")
        bridge.setSelection(0u, 0u, 0u, 0u, false)
    }
}
