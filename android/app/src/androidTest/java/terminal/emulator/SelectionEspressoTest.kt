package terminal.emulator

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * Selection flows through the ViewModel + the Surface-side PopupWindow menu
 * (复制/分享/全选/粘贴） — not the system ActionMode toolbar. The popup hosts
 * real TextViews, so menu items are asserted via UiAutomator (By.text).
 */
class SelectionEspressoTest {
    companion object {
        // 选择菜单是独立系统窗口：慢模拟器上无障碍树同步与首帧渲染滞后，
        // 5s 等待偶发超时，提到与落格门控同量级的 15s。
        private const val MENU_POPUP_TIMEOUT_MS = 15_000L

        /** 选区所在视口行（0 基）与起始列：与网格列数无关的固定量。 */
        private const val SELECTION_ROW = 2
        private const val SELECTION_START_COL = 10
    }

    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    /** 共用会话跨全部用例留存：本类直写网格，不清场即污染后继。 */
    @After
    fun resetSession() = composeTestRule.cleanUpTerminalState()

    /**
     * 建一段确定的部分选区，返回选区**实际**末列。
     *
     * 末列由网格列数派生而非写死：网格宽度随设备与主字体变化（实测 25～38 列），
     * 写死值只在其余设备上偶然相符——主字体一变，选区被钳到 `cols-1`，
     * 断言却仍按旧列数比对，判红的原因与选区功能无关。
     */
    private fun startPartialSelection(): Int {
        // 确定性内容：shell 自然行的内容不可控（空行则 selectedText 为空、菜单无复制按钮），
        // 直写长行到视口第 2 行并等落格，再选固定区间。
        terminal.emulator.UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        // 填满整行：行长恰为网格列数，故在任何列数下都不折行，整串 contains 恒成立。
        val maxCol =
            (composeTestRule.activity.terminalViewModel.runtime.state.value.cols - 1)
                .coerceAtLeast(SELECTION_START_COL + 1)
        val fill = "P".repeat(maxCol + 1)
        bridge.feedTerminal("\u001B[${SELECTION_ROW + 1};1H$fill".toByteArray(Charsets.UTF_8))
        checkNotNull(
            terminal.emulator.UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                composeTestRule.getBridge()?.getTerminalText()?.contains(fill) == true
            },
        ) { "填充行必须落格: [${fill.length} 字]" }
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.startSelection(SELECTION_ROW, SELECTION_START_COL)
            activity.terminalViewModel.updateSelection(SELECTION_ROW, maxCol)
            activity.terminalViewModel.endSelection()
        }
        composeTestRule.waitForIdle()
        return maxCol
    }

    @Test
    fun terminalContentIsDisplayed() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun partialSelectShowsSelectionMenu() {
        composeTestRule.waitForSession()
        startPartialSelection()
        // The selection menu is the app PopupWindow (platform text comes
        // from R.string.copy/share/select_all) — visible to UiAutomator.
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        assertTrue(
            "复制 action must be in the selection menu",
            device.wait(Until.hasObject(By.text("复制")), MENU_POPUP_TIMEOUT_MS),
        )
        assertTrue(
            "全选 action must be in the selection menu",
            device.wait(Until.hasObject(By.text("全选")), MENU_POPUP_TIMEOUT_MS),
        )
    }

    @Test
    fun selectAllShowsSelectionMenu() {
        composeTestRule.waitForSession()
        // 全量内容断言（对标 selectAllFromToolbarSelectsWholeBuffer）：先送显三行
        // 唯一标记，全选后 selectedText 必须全部包含，不止菜单出现。
        val stamp = System.currentTimeMillis() % 100000
        val markers = listOf("SELL_ALL_A_$stamp", "SELL_ALL_B_$stamp", "SELL_ALL_C_$stamp")
        // 桥单次读取：会话孵化中为 null，由调用方轮询重试（getBridge 契约）。
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        // house 模式：桥每次现取（Activity 重建/会话切换会替换桥实例，缓存实例
        // 读到的是旧会话）。送显与轮询都用现取桥；若中途切换导致标记丢失则补送。
        fun freshBridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        // vt_write 是裸 VT 路径（不做 \n→\r\n 转换，见 pty_write 注释）：
        // 裸 \n 只换行不回车，多行负载逐行右漂并在行尾从中间截断标记，
        // 使 contains 永远失败。用 \r\n 使每行自列 0 起笔。
        val payload = (markers.joinToString("\r\n") + "\r\n").toByteArray(Charsets.UTF_8)
        var fed = false
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 500) {
                runCatchingCancellable { terminal.emulator.bridge.NativeBridge.pollEvent() }
                val text = composeTestRule.getBridge()?.getTerminalText().orEmpty()
                if (markers.all { text.contains(it) }) {
                    true
                } else {
                    // 补送幂等：同一标记重复送显不影响 contains 断言。
                    fed = (runCatchingCancellable { freshBridge().feedTerminal(payload) }.getOrDefault(false)) || fed
                    false
                }
            }
        assertNotNull("标记必须落格", settled)
        assertTrue("标记送显失败", fed)
        // 补送循环每 500ms 就追加三行，慢机上缓冲区会被撑到网格底部。全选覆盖整屏时
        // 菜单无处可放——DESIGN.md:171「弹出菜单始终不遮挡被选择文本」要求此时隐藏，
        // 于是下面「菜单必须出现」的断言会随内容高度时红时绿。清屏后只送一次，使内容
        // 恒为三行、菜单必然放得下；断言本身不动。
        val cleared =
            runCatchingCancellable {
                freshBridge().feedTerminal("\u001B[2J\u001B[3J\u001B[H".toByteArray(Charsets.UTF_8))
            }.getOrDefault(false)
        assertTrue("清屏失败", cleared)
        assertTrue("清屏后二次送显失败", runCatchingCancellable { freshBridge().feedTerminal(payload) }.getOrDefault(false))
        val resettled =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 500) {
                runCatchingCancellable { terminal.emulator.bridge.NativeBridge.pollEvent() }
                val text = composeTestRule.getBridge()?.getTerminalText().orEmpty()
                markers.all { marker -> text.contains(marker) }
            }
        assertNotNull("清屏后标记必须重新落格", resettled)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.selectAll()
        }
        composeTestRule.waitForIdle()
        var selectedText = ""
        composeTestRule.activityRule.scenario.onActivity { activity ->
            selectedText = activity.terminalViewModel.state.value.selection.selectedText
        }
        for (marker in markers) {
            assertTrue("全选必须包含整缓冲区内容 [$marker], 实际=[$selectedText]", selectedText.contains(marker))
        }
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        assertTrue(
            "Selection menu must appear after Select All",
            device.wait(Until.hasObject(By.text("复制")), MENU_POPUP_TIMEOUT_MS),
        )
        // 全选后重锚（design 决策 3）：选择自视口顶起时上方无位，菜单必须翻到
        // 选择底缘之下（窗口 y ≥ 选择底缘），任何时刻不遮挡选择。
        var popupY = Int.MIN_VALUE
        var selectionBottomWindowY = Int.MIN_VALUE
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            val location = IntArray(2)
            surface.getLocationInWindow(location)
            val selection = activity.terminalViewModel.state.value.selection
            val endRow = selection.end?.row ?: -1
            val bridge = composeTestRule.getBridge()
            val depth = bridge?.scrollbackLength() ?: -1
            val density = activity.resources.displayMetrics.density
            val cellHeightPx = (bridge?.getCellHeight() ?: 0f) * density
            val scrollOffset = activity.terminalViewModel.runtime.activeSessionScrollOffset()
            val visibleBottomRow = endRow - (depth - scrollOffset)
            selectionBottomWindowY = location[1] + ((visibleBottomRow + 1) * cellHeightPx).toInt()
            val menuField =
                terminal.emulator.ui.TerminalSurface::class.java.getDeclaredField("selectionMenuPopup")
            menuField.isAccessible = true
            val popup = menuField.get(surface) as android.widget.PopupWindow?
            // PopupWindow 无 y getter：内容视图相对自身窗口恒为 [0,0]，
            // 故取屏幕坐标（与 surface 的窗口坐标同为屏幕系，可比）。
            val contentLocation = IntArray(2)
            popup?.contentView?.getLocationOnScreen(contentLocation)
            val surfaceScreenLocation = IntArray(2)
            surface.getLocationOnScreen(surfaceScreenLocation)
            selectionBottomWindowY =
                surfaceScreenLocation[1] + ((visibleBottomRow + 1) * cellHeightPx).toInt()
            popupY = if (popup?.contentView == null) Int.MIN_VALUE else contentLocation[1]
        }
        assertTrue(
            "全选后菜单必须重锚到选区下方 (menuY=$popupY bottomY=$selectionBottomWindowY)",
            popupY >= selectionBottomWindowY,
        )
    }

    @Test
    fun emptyAreaLongPressShowsPasteSelection() {
        composeTestRule.waitForSession()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.showPastePopup(10, 0)
        }
        composeTestRule.waitForIdle()
        // A paste-only selection state (the PasteChipOverlay was removed; an
        // empty-area long-press shows a paste-only selection + menu).
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            assertTrue("Expected a paste-only selection", sel.pasteOnly)
        }
    }

    @Test
    @SuppressLint("DeprecatedCall") // primaryClip: no @Deprecated in API 37; slack-lint rule data lag
    fun copyActionPlacesTextOnClipboard() {
        composeTestRule.waitForSession()
        // 确定性内容：经 shell 回显送显依赖回显链时序，慢机上 3s 裸睡不够；
        // 直写解析器并等落格（与 selectAllShowsSelectionMenu 同口径）。
        terminal.emulator.UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        val marker = "copy-me-selection-target"
        bridge.feedTerminal("$marker\r\n".toByteArray(Charsets.UTF_8))
        terminal.emulator.UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
            composeTestRule.getBridge()?.getTerminalText()?.contains(marker) == true
        }
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.terminalViewModel.selectAll()
        }
        composeTestRule.waitForIdle()

        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val copy = device.wait(Until.findObject(By.text("复制")), MENU_POPUP_TIMEOUT_MS)
        assertTrue("复制 action must be present", copy != null)
        requireNotNull(copy).click()
        // Clipboard write happens on the native side after the action callback
        // — poll for it.
        var clipboardReady = false
        val deadline = System.currentTimeMillis() + 5_000
        while (!clipboardReady && System.currentTimeMillis() < deadline) {
            Thread.sleep(100)
            composeTestRule.activityRule.scenario.onActivity { activity ->
                val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                clipboardReady = clipboard.primaryClip != null
            }
        }
        assertTrue("Clipboard should contain a clip after Copy", clipboardReady)
    }

    @Test
    fun selectionStateIsActiveAfterPartialSelect() {
        composeTestRule.waitForSession()
        val expectedEndCol = startPartialSelection()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val sel = activity.terminalViewModel.state.value.selection
            assertTrue("Selection should be active", sel.active)
            val start = requireNotNull(sel.start)
            val end = requireNotNull(sel.end)
            assertEquals(SELECTION_ROW, start.row)
            assertEquals(SELECTION_START_COL, start.col)
            assertEquals(expectedEndCol, end.col)
        }
    }
}
