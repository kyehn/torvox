package terminal.emulator.ui

import android.content.ClipboardManager
import android.content.Context
import android.os.SystemClock
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.injectDoubleTap
import terminal.emulator.injectTap
import terminal.emulator.injectTripleTap
import terminal.emulator.waitForSession

/**
 * 多击选择端到端（对标 sylirre TerminalUiTest.doubleTapSelectsWord/tripleTapSelectsLine）：
 * 双击选词、三击选行，复制动作把选中文本填入系统剪贴板。
 *
 * 确定性放置：标记经 feedTerminal 直写 VT（\e[2J 清屏 + CUP 定位到中带行，
 * 不经 shell 行编辑，shell 空闲不输出，网格稳定），点击坐标由标记落格行与
 * 物理单元格度量（bridge 逻辑值 × density，运行时触摸数学同口径）算出，
 * 为视图本地坐标（注入直达 surface dispatchTouchEvent）。菜单为应用内
 * PopupWindow（中文“复制”，R.string.copy；应用仅支持简体中文），与真实长按同路径。
 */
@RunWith(JUnit4::class)
class MultiTapSelectionInstrumentedTest {
    companion object {
        private const val GRID_TIMEOUT_MS = 15_000L
        /** 输出静默窗口：shell 启动输出落定后才送显标记。 */
        private const val QUIET_WINDOW_MS = 2_000L
        private const val MENU_TIMEOUT_MS = 5_000L
        private const val CLIPBOARD_TIMEOUT_MS = 5_000L
        /** 0 基中带行：24 行视口第 13 行，上下留白，IME 与状态栏均不干扰。 */
        private const val MARKER_VIEWPORT_ROW = 12
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        // 桥单次读取：会话孵化中为 null，由调用方轮询重试（getBridge 契约）。
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
        settleKeyboard()
    }

    /**
     * 键盘预热：首击 surface 把 IME 拉起并等动画落定。慢模拟器上 IME
     * 显隐翻转晚到 1.5s+，若与点选手势竞态，翻转会清掉刚建的选择
     * （TerminalSurface onApplyWindowInsets 翻转门限）；预热后手势期无翻转。
     * 这也是真实用户场景（键盘已弹起再双击选词）。
     */
    private fun settleKeyboard() {
        val surface = findTerminalSurface(composeTestRule.activity)
        // 中部点击：只聚焦 + 拉键盘，不落选择（单击不建选择）。
        injectTap(surface, surface.width / 2f, surface.height / 2f)
        val shown =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
                var visible = false
                InstrumentationRegistry.getInstrumentation().runOnMainSync {
                    visible = composeTestRule.activity.window.decorView.rootWindowInsets
                        ?.isVisible(android.view.WindowInsets.Type.ime()) == true
                }
                visible
            }
        assertNotNull("IME 必须弹起（20s 未可见）", shown)
        // 动画尾帧 inset 仍在漂：多等一拍，翻转门限彻底落定。
        Thread.sleep(1_500)
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /**
     * 经 feedTerminal 把 [marker] 放到中带行，返回词内点击的视图本地坐标与
     * 标记行实际全文（行尾可能有 shell 尾巴，三击断言用实测行）。
     * [tapColInMarker] 为词内列偏移（落在词内即选中整词）。
     */
    private fun feedMarkerAndTapPosition(marker: String, tapColInMarker: Int): Triple<Float, Float, String> {
        // 桥每次现取：会话孵化/重启会替换运行时桥实例，缓存实例读到的是
        // 旧会话（文本永不更新，表现为 prompt/标记永不出现）。
        // 持续泵送事件队列：运行时会话的输出收割/查询可见性需要 pollEvent
        // 驱动（StickyCtrl/VtCorrectness 同模式），否则文本永不更新。
        fun currentText(): String? {
            runCatching { NativeBridge.pollEvent() }
            return composeTestRule.getBridge()?.getTerminalText()
        }
        // 等输出静默（shell 启动输出落定）：若在标记送显后 shell 再输出，
        // 会追加到标记行，污染词界与整行断言。判 prompt 字符不可靠
        // （提示符样式/落定抖动），直接判文本稳定。
        var lastText = currentText()
        var quietSince = SystemClock.uptimeMillis()
        val quiet =
            UxTestUtils.pollUntilTrue(timeoutMs = 60_000, intervalMs = 200) {
                val current = currentText()
                if (current != lastText) {
                    lastText = current
                    quietSince = SystemClock.uptimeMillis()
                    false
                } else {
                    SystemClock.uptimeMillis() - quietSince > QUIET_WINDOW_MS
                }
            }
        assertNotNull("终端输出未静默", quiet)
        // 标记从列 0 起笔（整行选择断言要求行内无缩进空格）；点击列由调用方
        // 选在词中部（抽屉边缘区 32dp≈4 列之外），注：CUP 列参数实测不生效
        // （行生效），列 0 起笔即可。shell 已静默，无尾巴污染。
        val fed = bridge().feedTerminal("\u001B[2J\u001B[13;1H$marker".toByteArray(Charsets.UTF_8))
        assertTrue("标记送显失败", fed)
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.contains(marker) == true
            }
        assertNotNull("标记必须落格: $marker", seen)
        // getTerminalText 拼接 scrollback+visible：减回滚深度换算视口行。
        val depth = bridge().scrollbackLength()
        val lines = currentText().orEmpty().lines()
        val index = lines.indexOfFirst { it.contains(marker) }
        assertTrue("标记行定位失败: $marker", index >= 0)
        val viewportRow = index - depth
        assertTrue("标记必须在可见视口内 (行=$viewportRow, 深度=$depth)", viewportRow >= 0)
        val col = lines[index].indexOf(marker) + tapColInMarker
        // 物理单元格（运行时触摸数学同口径：逻辑值 × density）。
        val density = composeTestRule.activity.resources.displayMetrics.density
        val cellWidth = bridge.getCellWidth() * density
        val cellHeight = bridge.getCellHeight() * density
        assertTrue("单元格度量不可用 ($cellWidth x $cellHeight)", cellWidth > 0f && cellHeight > 0f)
        val tapX = (col + 0.5f) * cellWidth
        // surface 左侧 32dp 为抽屉边缘区（触摸直达丢弃）：断言点击在其外。
        assertTrue("点击必须在抽屉边缘区外 (x=$tapX)", tapX > 32f * composeTestRule.activity.resources.displayMetrics.density)
        return Triple(tapX, (viewportRow + 0.5f) * cellHeight, lines[index].trim())
    }

    /** 当前选择快照（断言分段定位：手势/选择 vs 菜单/复制）。 */
    private fun selectionSnapshot(): Triple<Boolean, String, Boolean> {
        var snapshot = Triple(false, "", false)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val selection = activity.terminalViewModel.state.value.selection
            snapshot = Triple(selection.active, selection.selectedText, selection.menuDismissed)
        }
        return snapshot
    }

    private fun copyMenuText(): String = composeTestRule.activity.getString(terminal.emulator.R.string.copy)

    private fun awaitCopyMenu() {
        val menu = device.wait(Until.findObject(By.text(copyMenuText())), MENU_TIMEOUT_MS)
        assertNotNull("多击后选择菜单必须出现", menu)
        menu.click()
    }

    private fun clipboardText(): String {
        var text = ""
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val clipboard =
                activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            text = clipboard.primaryClip?.getItemAt(0)?.coerceToText(activity)?.toString().orEmpty()
        }
        return text
    }

    @Test
    fun doubleTapSelectsWordAndCopyFillsClipboard() {
        val word = "MTAPW_${System.currentTimeMillis() % 100000}"
        // 点词中部：x≈6.5列宽，远在抽屉边缘区外。
        val (tapX, tapY, _) = feedMarkerAndTapPosition(word, tapColInMarker = 5)
        injectDoubleTap(findTerminalSurface(composeTestRule.activity), tapX, tapY)
        // raw 触摸不经过 compose 同步通道：waitForIdle 排空重组器，
        // 否则选择菜单的 LaunchedEffect 不执行（诊断确认）。
        composeTestRule.waitForIdle()
        val (active, selectedText, dismissed) = selectionSnapshot()
        assertTrue("双击后选择必须激活 (active=$active text=[$selectedText] dismissed=$dismissed)", active)
        assertTrue("双击必须选中整词 (实际=[$selectedText] 期望=[$word])", selectedText == word)
        awaitCopyMenu()
        // 复制动作把选中的词填入系统剪贴板。
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = CLIPBOARD_TIMEOUT_MS, intervalMs = 100) {
                clipboardText() == word
            }
        assertNotNull("复制后剪贴板必须为选中的词, 实际: [${clipboardText()}]", settled)
    }

    @Test
    fun tripleTapSelectsLine() {
        val stamp = System.currentTimeMillis() % 100000
        val marker = "MTAPA_$stamp MTAPB_$stamp"
        // 点第二词内：整行选择与落点词无关，但落点须在边缘区外。
        val (tapX, tapY, expectedLine) = feedMarkerAndTapPosition(marker, tapColInMarker = marker.indexOf("MTAPB") + 2)
        injectTripleTap(findTerminalSurface(composeTestRule.activity), tapX, tapY)
        // 同双击：排空重组器，菜单 LaunchedEffect 才会执行。
        composeTestRule.waitForIdle()
        val (active, selectedText, dismissed) = selectionSnapshot()
        assertTrue("三击后选择必须激活 (active=$active text=[$selectedText] dismissed=$dismissed)", active)
        assertTrue("三击必须选中整行 (实际=[$selectedText] 期望=[$expectedLine])", selectedText == expectedLine)
        awaitCopyMenu()
        // 三击选整行：剪贴板必须为该行全文。
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = CLIPBOARD_TIMEOUT_MS, intervalMs = 100) {
                clipboardText() == expectedLine
            }
        assertNotNull("复制后剪贴板必须为整行, 实际: [${clipboardText()}] 期望: [$expectedLine]", settled)
    }
}
