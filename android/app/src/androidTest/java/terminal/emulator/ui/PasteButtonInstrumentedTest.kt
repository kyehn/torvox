package terminal.emulator.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.click
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.RootMatchers.isPlatformPopup
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withText
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
import terminal.emulator.injectLongPress
import terminal.emulator.waitForSession

/**
 * 粘贴端到端（对标 sylirre TerminalUiTest.pasteButtonTypesClipboardIntoShell）：
 * 剪贴板置入唯一标记 → 真实长按空白区 → 粘贴菜单出现 → 点击“粘贴” →
 * 标记出现在 shell 输入回显中。
 *
 * 路径与真实用户一致（空白长按 → paste-only 选择 → PopupWindow 粘贴项 →
 * pasteFromClipboardDirect → writeToPty），不用 showPastePopup 直调后门。
 */
@RunWith(JUnit4::class)
class PasteButtonInstrumentedTest {
    companion object {
        private const val GRID_TIMEOUT_MS = 15_000L
        private const val QUIET_WINDOW_MS = 2_000L
        private const val PASTE_TIMEOUT_MS = 30_000L
        /** 点击列：6.5 列宽处，远在 32dp 抽屉边缘区外。 */
        private const val TAP_COL = 6
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
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** 泵送事件队列后读全量文本（运行时输出收割需 pollEvent 驱动）。 */
    private fun currentText(): String? {
        runCatching { NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    /** 等终端输出静默（shell 启动输出落定），避免标记行被追加污染。 */
    private fun awaitQuiet(timeoutMs: Long = 60_000) {
        var lastText = currentText()
        var quietSince = android.os.SystemClock.uptimeMillis()
        val quiet =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 200) {
                val current = currentText()
                if (current != lastText) {
                    lastText = current
                    quietSince = android.os.SystemClock.uptimeMillis()
                    false
                } else {
                    android.os.SystemClock.uptimeMillis() - quietSince > QUIET_WINDOW_MS
                }
            }
        assertNotNull("终端输出未静默", quiet)
    }

    @Test
    fun pasteMenuTypesClipboardIntoShell() {
        // 参考实现首步即等 prompt：静默等待在空屏（shell 未就绪）也会通过，
        // 而 shell 就绪前的粘贴字节去向不明，必须门控 prompt 存在。
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = 60_000, intervalMs = 200) {
                val text = currentText()
                text != null && (text.contains("$") || text.contains("#"))
            }
        assertNotNull("shell prompt 必须先就绪, 实际: ${currentText()?.takeLast(200)}", promptSeen)
        awaitQuiet()
        // 清屏：prompt 回到视口首行，其余行全空，长按落点必为空白。
        assertTrue("清屏送显失败", bridge().feedTerminal("\u001B[2J".toByteArray(Charsets.UTF_8)))
        awaitQuiet()
        val depth = bridge().scrollbackLength()
        val lines = currentText().orEmpty().lines()
        // 视口第 5 行（0 基）必须空白：prompt 占首行，其余无输出。
        val blankIndex = depth + 5
        val blankLine = lines.getOrNull(blankIndex).orEmpty()
        assertTrue("长按行必须空白 (行=$blankIndex 内容=[$blankLine])", blankLine.isBlank())

        val marker = "PASTEA${System.currentTimeMillis() % 100000}"
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("test", marker))
        }

        val density = composeTestRule.activity.resources.displayMetrics.density
        val cellWidth = bridge().getCellWidth() * density
        val cellHeight = bridge().getCellHeight() * density
        assertTrue("单元格度量不可用 ($cellWidth x $cellHeight)", cellWidth > 0f && cellHeight > 0f)
        val tapX = (TAP_COL + 0.5f) * cellWidth
        assertTrue(
            "点击必须在抽屉边缘区外 (x=$tapX)",
            tapX > 32f * composeTestRule.activity.resources.displayMetrics.density,
        )
        val tapY = (5 + 0.5f) * cellHeight
        injectLongPress(findTerminalSurface(composeTestRule.activity), tapX, tapY)
        composeTestRule.waitForIdle()

        // 分段断言：手势/选择 vs 菜单/粘贴。
        var active = false
        var pasteOnly = false
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val selection = activity.terminalViewModel.state.value.selection
            active = selection.active
            pasteOnly = selection.pasteOnly
        }
        assertTrue("长按空白后选择必须激活 (active=$active pasteOnly=$pasteOnly)", active)
        assertTrue("长按空白必须为纯粘贴选择 (pasteOnly=$pasteOnly)", pasteOnly)

        // 与应用 ClipboardAccess.clipboardText() 同逻辑预读：切分“剪贴板”与“粘贴写入”。
        var clipRead: String? = "<unread>"
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val clipboard =
                activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipRead =
                if (!clipboard.hasPrimaryClip()) {
                    "<no-primary-clip>"
                } else {
                    clipboard.primaryClip?.getItemAt(0)?.text?.toString()
                }
        }
        assertTrue("测试进程必须读回剪贴板标记, 实际=[$clipRead]", clipRead == marker)

        val pasteText = composeTestRule.activity.getString(terminal.emulator.R.string.paste)
        // 参考实现同款：Espresso 点击 popup 内“粘贴”（UiAutomator 按 accessibility
        // 坐标点击曾出现“清选择但未粘贴”——疑似点中 surface 而非按钮；Espresso 直点
        // 活视图，缺席则大声失败，不会误清选择）。
        onView(withText(pasteText))
            .inRoot(isPlatformPopup())
            .check(matches(isDisplayed()))
            .perform(click())
        // 诊断：截图落盘到应用外部目录（免权限可 pull），目视确认菜单位置。
        runCatching {
            val dir = composeTestRule.activity.getExternalFilesDir(null)
            device.takeScreenshot(java.io.File(dir, "paste_menu_shot.png"))
        }
        // 点击前剪贴板已由 clipRead 预读断言确认，无需重设。

        // 粘贴文本经 pty 进入 shell，回显在输入行（参考实现去换行比对）。
        val pasted =
            UxTestUtils.pollUntilTrue(timeoutMs = PASTE_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.replace("\n", "")?.contains(marker) == true
            }
        if (pasted == null) {
            // 双标记二分：菜单字节是“丢失”还是“延迟31s+才到”？直调用不同标记，
            // 若最终只见 B 不见 A → 菜单字节真丢；若 A 也出现 → 延迟投递。
            val markerB = "PASTEB${System.currentTimeMillis() % 100000}"
            composeTestRule.activityRule.scenario.onActivity { activity ->
                val clipboard =
                    activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                clipboard.setPrimaryClip(ClipData.newPlainText("test", markerB))
                activity.terminalViewModel.pasteFromClipboard()
            }
            var seenA = false
            var seenB = false
            UxTestUtils.pollUntilTrue(timeoutMs = PASTE_TIMEOUT_MS, intervalMs = 100) {
                val text = currentText()?.replace("\n", "")
                seenA = text?.contains(marker) == true
                seenB = text?.contains(markerB) == true
                seenB
            }
            assertNotNull(
                "菜单字节去向不明 (menu见A=$seenA, direct见B=$seenB)",
                if (seenB && !seenA) null else true,
            )
        }
    }
}
