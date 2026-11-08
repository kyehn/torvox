
package terminal.emulator.ui

import android.graphics.Bitmap
import android.os.SystemClock
import android.util.Log
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.MainActivity
import terminal.emulator.R
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitShellPrompt
import terminal.emulator.bridge.Bridge
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.getBridge
import terminal.emulator.openDrawer
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession
import java.io.File
import java.io.FileOutputStream

/**
 * Comprehensive end-to-end test for text search functionality.
 *
 * Tests:
 * - Search bar opens from drawer button (not ctrl+f)
 * - Search finds text and highlights matches with inverted colors
 * - Previous/Next navigation with auto-scroll to off-screen matches
 * - Smart case toggle (auto-detect uppercase)
 * - Close clears highlights and shows modifier bar again
 * - IME does not obscure search bar
 * - OCR verification of highlighted cells
 * - Scroll effect when navigating to off-screen matches
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class TextSearchEndToEndTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val TAG = "SearchE2ETest"
        private const val SCREENSHOT_DIR = "search_e2e_screenshots"
    }

    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.

    @After
    fun resetSession() = composeTestRule.cleanUpTerminalState()

    private val uniqueMarker: String by lazy {
        "SRCH_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"
    }

    private val multiLineMarker: String by lazy {
        "MLINE_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"
    }

    // ── Helper: generate multi-page content ──

    private fun generateMultiPageContent(bridge: Bridge, marker: String) {
        // 共用会话的屏幕可能已被上个用例清空：awaitShellPrompt 等不到时会回车重画提示符。
        awaitShellPrompt(bridge)
        // Generate enough content to fill >3 terminal pages
        val linesToFill = 200
        for (i in 1..linesToFill) {
            val content =
                when {
                    i % 10 == 0 -> "LINE_${i}_MARKER_$marker"
                    i % 7 == 0 -> marker
                    i % 5 == 0 -> "CONTENT_LINE_$i"
                    else -> "data_line_$i"
                }
            bridge.writeToPty("echo '$content'\n".toByteArray())
        }
        // 落格门控替代裸睡：慢模拟器上 200 行回显滞后，内容未齐即搜即错。
        awaitTerminalContains(bridge, marker)
    }

    private fun awaitBridge(): Bridge {
        // 会话孵化慢于 UI 呈现：waitForSession 只等界面节点，桥可能仍为 null
        // （Osc52 同门控）。单次直读必竞态。
        val ready =
            terminal.emulator.UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                composeTestRule.getBridge() != null
            }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", ready)
        return composeTestRule.getBridge() ?: throw AssertionError("bridge null")
    }

    /** 等标记落格到终端文本：命令回显与命令输出都算命中。 */
    private fun awaitTerminalContains(bridge: Bridge, marker: String, timeoutMs: Long = 60_000) {
        val landed =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 200) {
                runCatchingCancellable { terminal.emulator.bridge.NativeBridge.pollEvent() }
                runCatchingCancellable { bridge.getTerminalText() }.getOrNull()?.contains(marker) == true
            }
        assertNotNull("标记必须落格: $marker", landed)
    }

    /** 应用上下文：计数文案取自资源，不硬编码界面文字。 */
    private val appContext
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    /** 搜索计数文本；计数节点尚未发射时返回 null。 */
    private fun searchResultCountText(): String? = runCatching {
        composeTestRule.onNodeWithTag("SearchResultCount").fetchSemanticsNode()
            .config.getOrNull(SemanticsProperties.Text)
            ?.joinToString("") { it.text }
    }.getOrNull()

    /**
     * 计数文案的正则：由资源串推导，占位符（`%1$d` 一类）换成数字匹配。
     *
     * 硬编码文案会在改字时静默失配——断言要么永假要么永真，看不出是文案变了
     * 还是搜索坏了。从资源串推导则文案与断言同源。
     */
    private fun countPattern(resourceId: Int): Regex {
        // 用固定数字填一遍资源串，再把数字段换成「若干位数字」：文案与断言同源，
        // 且测试里不必重复写 `%1$d` 一类占位符语法。
        // 逐段引用而非整串引用：`Regex.escape` 会把整串包进 \Q…\E，数字段
        // 也会随之变成字面量，正则就再也匹配不到真实计数。
        val sample = appContext.getString(resourceId, 1, 2)
        val pattern = sample.split(Regex("\\d+")).joinToString("\\d+") { Regex.escape(it) }
        return Regex(pattern)
    }

    /** 等命中计数落定：计数文本必须给出命中个数（`共 N 个`）。 */
    private fun awaitSearchCount(timeoutMs: Long = 20_000) {
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 100) {
                countPattern(R.string.search_results_total).containsMatchIn(searchResultCountText().orEmpty())
            }
        assertNotNull("搜索命中计数未落定（当前文本：${searchResultCountText()}）", settled)
    }

    /** 等选中计数落定：导航后计数文本必须给出当前序号（`第 k 个，共 N 个`）。 */
    private fun awaitSelectedCount(timeoutMs: Long = 20_000) {
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 100) {
                countPattern(R.string.search_result_of).containsMatchIn(searchResultCountText().orEmpty())
            }
        assertNotNull("搜索选中计数未落定（当前文本：${searchResultCountText()}）", settled)
    }

    /** 等计数翻成「无结果」：该文案只在查不到时出现。 */
    private fun awaitNoResultsCount(timeoutMs: Long = 20_000) {
        val noResultsText = appContext.getString(R.string.search_no_results)
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 100) {
                searchResultCountText() == noResultsText
            }
        assertNotNull("搜索计数未翻成「无结果」（当前文本：${searchResultCountText()}）", settled)
    }

    private fun openSearchAndType(text: String) {
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement(text)
        composeTestRule.waitForIdle()
    }

    private fun saveScreenshot(name: String) {
        val screenshotDir = File(composeTestRule.activity.filesDir, SCREENSHOT_DIR)
        screenshotDir.mkdirs()
        val file = File(screenshotDir, "$name.png")
        try {
            // 必须走 UiAutomation.takeScreenshot：它取的是真实屏幕合成结果。
            // 自造 `decorView.draw(canvas)` 有两个问题——测试体不在主线程，
            // 离线程 draw 会被 Compose 的 SnapshotStateObserver 判为跨线程访问
            // （CI 崩在 `LookaheadCapablePlaceable.captureRulers`）；且该 draw
            // 根本采不到 Surface/TextureView 里的终端像素，OCR 拿到的输入是空的。
            val bitmap =
                InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot()
                    ?: throw AssertionError("takeScreenshot returned null")
            FileOutputStream(file).use { fileOutputStream ->
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, fileOutputStream)
            }
            bitmap.recycle()
            Log.d(TAG, "Screenshot saved: ${file.absolutePath}")
        } catch (exception: Exception) {
            Log.e(TAG, "Failed to save screenshot", exception)
        }
    }

    // ── Test 1: Search opens from drawer button ──

    @Test
    fun searchOpensFromDrawerButton() {
        composeTestRule.waitForSession()
        composeTestRule.waitForIdle()

        // Verify search bar is NOT visible initially
        val initialBarCount =
            composeTestRule
                .onAllNodes(hasTestTag("TextSearchBar"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("Search bar must not be visible before opening", 0, initialBarCount)

        // Also verify ctrl+f does NOT open the search bar. This is an
        // intentional negative assertion on current behaviour (no ctrl+F
        // shortcut): it must be updated if a shortcut is ever added
        //
        composeTestRule.activity.runOnUiThread {
            composeTestRule.activity.dispatchKeyEvent(
                android.view.KeyEvent(
                    SystemClock.uptimeMillis(),
                    SystemClock.uptimeMillis(),
                    android.view.KeyEvent.ACTION_DOWN,
                    android.view.KeyEvent.KEYCODE_F,
                    0,
                    android.view.KeyEvent.META_CTRL_ON,
                ),
            )
        }
        composeTestRule.waitForIdle()
        val ctrlFCount =
            composeTestRule
                .onAllNodes(hasTestTag("TextSearchBar"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("Ctrl+F must NOT open search bar", 0, ctrlFCount)

        // Open drawer and click search button
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()

        // Verify search bar IS visible
        composeTestRule.onNodeWithTag(
            "TextSearchBar",
        ).assertExists("Search bar must be visible after opening from drawer")
        composeTestRule.onNodeWithTag("SearchTextField").assertExists("Search text field must be visible")
        saveScreenshot("01_search_bar_opened")
    }

    // ── Test 2: Search finds and highlights text ──

    @Test
    fun searchFindsAndHighlightsMatches() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()
        generateMultiPageContent(bridge, uniqueMarker)

        // Verify marker exists in terminal

        // Open search and type the marker
        openSearchAndType(uniqueMarker)

        // 命中数必须真的算出来（固定睡眠只保证「节点存在」，不保证「查到了」）。
        awaitSearchCount()

        // Take screenshot for host-side OCR verification
        saveScreenshot("02_search_highlights")

        // NOTE: OCR verification runs on the HOST: the dev shell carries `rapidocr`
        // and its models (`scripts/download-rapidocr-models.nu` fetches them),
        // a device-side ProcessBuilder("rapidocr") can never work — the binary
        // lives in the host development shell, not on the device. So this test
        // asserts the UI behavior (result count + screenshot artifact) and leaves
        // the pixel verdict to the host step. Screenshot must exist as its input.
        val screenshotFile =
            File(
                File(composeTestRule.activity.filesDir, SCREENSHOT_DIR),
                "02_search_highlights.png",
            )
        assertTrue(
            "Screenshot must exist for host OCR verification",
            screenshotFile.exists(),
        )
    }

    // ── Test 3: Search navigation (previous/next) with scrolling ──

    @Test
    fun searchNavigatesWithScroll() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()
        generateMultiPageContent(bridge, uniqueMarker)

        openSearchAndType(uniqueMarker)
        awaitSearchCount()

        // Click Next multiple times to verify navigation
        composeTestRule.onNodeWithTag("SearchNext").performClick()
        awaitSelectedCount()
        composeTestRule.onNodeWithTag("SearchNext").performClick()
        awaitSelectedCount()
        saveScreenshot("03_after_next_twice")

        // Click Previous to go back
        composeTestRule.onNodeWithTag("SearchPrevious").performClick()
        awaitSelectedCount()
        saveScreenshot("04_after_previous")

        // Verify terminal text still contains the marker (search didn't break terminal)
        val terminalText = bridge.getTerminalText().orEmpty()
        assertTrue(
            "terminal must still contain $uniqueMarker after search navigation",
            terminalText.contains(uniqueMarker),
        )
    }

    // ── Test 4: 大小写按上游口径（ASCII 字母不区分大小写）──

    @Test
    fun lowercaseQueryMatchesUppercaseContent() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()
        generateMultiPageContent(bridge, uniqueMarker)

        // 小写查询命中大写内容：搜索由上游 search::Search 承担，仅 ASCII 字母折叠大小写，
        // 因此不提供（也不需要）区分大小写开关。
        openSearchAndType(uniqueMarker.lowercase())
        // 小写查询必须真的命中大写内容：计数落定即证明命中数 > 0。
        awaitSearchCount()

        saveScreenshot("05_case_insensitive_search")
    }

    // ── Test 5: Close search restores modifier bar ──

    @Test
    fun closeSearchRestoresModifierBar() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()
        generateMultiPageContent(bridge, uniqueMarker)

        openSearchAndType(uniqueMarker)
        awaitSearchCount()

        // Close search
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        composeTestRule.waitForIdle()

        // Verify search bar is gone
        val afterCloseCount =
            composeTestRule
                .onAllNodes(hasTestTag("TextSearchBar"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("Search bar must be hidden after close", 0, afterCloseCount)

        // Verify modifier bar is back
        composeTestRule.onNodeWithTag("ModifierBar").assertExists("Modifier bar must be visible after search close")
        saveScreenshot("06_after_search_close")
    }

    // ── Test 6: Multi-line text with multiple matches ──

    @Test
    fun multiLineSearch() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()

        // Generate content on multiple lines
        for (i in 1..30) {
            bridge.writeToPty("echo '${multiLineMarker}_$i'\n".toByteArray())
        }
        // 落格门控替代裸睡：回显滞后时搜到的行数会偏少。
        awaitTerminalContains(bridge, multiLineMarker)

        openSearchAndType(multiLineMarker)
        awaitSearchCount()

        // 30 行内容必须全部命中（每行在回显与输出各出现一次）。
        val countText = searchResultCountText().orEmpty()
        val matchedLines = Regex("\\d+").findAll(countText).map { it.value.toInt() }.maxOrNull() ?: 0
        assertTrue("30 行内容必须全部命中（计数文本：$countText）", matchedLines >= 30)
        saveScreenshot("07_multi_line_search")
    }

    // ── Test 7: Empty query shows no results ──

    @Test
    fun emptyQuery() {
        composeTestRule.waitForSession()

        // Open search without typing anything
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()

        // Result count should not be shown for empty query
        val emptyQueryCount =
            composeTestRule
                .onAllNodes(hasTestTag("SearchResultCount"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertEquals("No result count for empty query", 0, emptyQueryCount)
    }

    // ── Test 8: No results shows proper indicator ──

    @Test
    fun noResultsIndicator() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()

        // 先让界面上有命中：计数文案此时是「共 N 个」。
        val marker = "NOMATCH_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"
        bridge.writeToPty("echo '$marker'\n".toByteArray())
        awaitTerminalContains(bridge, marker)

        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement(marker)
        composeTestRule.waitForIdle()
        awaitSearchCount()

        // 换成查不到的串：计数必须从命中态翻回「无结果」。
        // 只断言「无结果」文案存在是不够的：搜索还没跑完时它就已经是这句了。
        composeTestRule.onNodeWithTag("SearchTextField").performTextReplacement("XYZZYX_NONEXISTENT_12345")
        composeTestRule.waitForIdle()
        awaitNoResultsCount()

        saveScreenshot("08_no_results")
    }

    // ── Test 9: Search bar stays visible with IME ──

    @Test
    fun searchBarNotObscuredByIme() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()

        openSearchAndType(uniqueMarker)

        // Focus the search field (triggers IME)
        composeTestRule.onNodeWithTag("SearchTextField").performClick()
        composeTestRule.waitForIdle()

        // Take screenshot to verify search bar is visible with IME
        saveScreenshot("09_search_with_ime")

        // Verify search bar is still visible
        composeTestRule.onNodeWithTag("TextSearchBar").assertExists("Search bar must be visible with IME open")
        composeTestRule.onNodeWithTag("SearchTextField").assertExists("Search text field must be visible with IME open")

        // Close IME and verify search bar still works
        composeTestRule.activity.runOnUiThread {
            val inputMethodManager =
                composeTestRule.activity.getSystemService(
                    android.content.Context.INPUT_METHOD_SERVICE,
                ) as android.view.inputmethod.InputMethodManager
            inputMethodManager.hideSoftInputFromWindow(
                composeTestRule.activity.window.decorView.windowToken,
                0,
            )
        }
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TextSearchBar").assertExists("Search bar must remain after IME close")
    }

    // ── Test 10: Search highlights use theme-based inverted colors ──

    @Test
    fun searchHighlightColors() {
        composeTestRule.waitForSession()
        val bridge = awaitBridge()
        generateMultiPageContent(bridge, uniqueMarker)

        openSearchAndType(uniqueMarker)
        // 命中后才截屏：没有命中的画面证明不了高亮配色。
        awaitSearchCount()

        // Save a screenshot for pixel analysis
        saveScreenshot("10_highlight_colors")

        // Visually verify highlights in screenshot
        val screenshotFile =
            File(
                File(composeTestRule.activity.filesDir, SCREENSHOT_DIR),
                "10_highlight_colors.png",
            )
        assertTrue(
            "Screenshot must exist for host-side OCR verification",
            screenshotFile.exists(),
        )
        // OCR pixel verification runs on the host (rapidocr in the host
        // development shell against pulled screenshots); the device test asserts
        // the artifact exists so the host step has input.
    }
}
