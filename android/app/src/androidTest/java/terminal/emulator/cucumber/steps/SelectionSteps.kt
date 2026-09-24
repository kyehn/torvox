package terminal.emulator.cucumber.steps

import android.annotation.SuppressLint
import android.content.ClipboardManager
import android.content.Context
import android.view.View
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import io.cucumber.java.zh_cn.假如
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import terminal.emulator.MainActivity
import terminal.emulator.SelectionAnchor
import terminal.emulator.UxTestUtils
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.injectLongPress
import terminal.emulator.waitForSession
import javax.inject.Inject

class SelectionSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    // 跨场景共享的胶水实例会常驻，字段只在同一场景内传递，每次使用前重写。
    private var dragBeforeEnd: SelectionAnchor? = null
    private var copiedText: String? = null

    companion object {
        // 会话孵化期写入重试上限与间隔：只补发被丢弃的写入，不延长输出轮询窗。
        private const val WRITE_RETRY_MAX = 3
        private const val WRITE_RETRY_INTERVAL_MS = 500L

        // 手柄覆盖窗视图类名：TerminalSurface$SelectionHandles$HandleOverlayLayout。
        private const val HANDLE_OVERLAY_CLASS =
            "terminal.emulator.ui.TerminalSurface\$SelectionHandles\$HandleOverlayLayout"
        private const val HANDLE_WAIT_TIMEOUT_MS = 10_000L
    }

    private fun surface(): View {
        val scenario = composeRuleHolder.composeRule.activityRule.scenario
        var surface: View? = null
        scenario.onActivity { activity ->
            surface = findTerminalSurface(activity)
        }
        return checkNotNull(surface) { "找不到终端 Surface" }
    }

    private fun selection(): terminal.emulator.SelectionState {
        var selection: terminal.emulator.SelectionState? = null
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            selection = (activity as MainActivity).terminalViewModel.state.value.selection
        }
        return checkNotNull(selection) { "读不到选择状态" }
    }

    // 往屏幕写满数字行并等末尾标记回显，保证长按落点有文本可供单词选中。
    private fun ensureScreenText() {
        val rule = composeRuleHolder.composeRule
        val bridge = rule.getBridge() ?: throw AssertionError("拿不到终端桥")
        val payload = "echo SELTEXT_START\nseq 1 50\necho SELTEXT_END\n".toByteArray(Charsets.UTF_8)
        // 会话仍在孵化时写入会被丢弃（writeToPty 返回 false）： bounded 重试，
        // 而不是把一次过早写入当成无输出。输出仍须在轮询窗内出现，不放宽断言。
        var written = bridge.writeToPty(payload)
        var attempts = 1
        while (!written && attempts < WRITE_RETRY_MAX) {
            Thread.sleep(WRITE_RETRY_INTERVAL_MS)
            written = bridge.writeToPty(payload)
            attempts++
        }
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                bridge.getTerminalText()?.contains("SELTEXT_END") == true
            }
        val snapshot = bridge.getTerminalText()
        assertNotNull(
            "终端未显示文本 written=$written attempts=$attempts textLen=${snapshot?.length} " +
                "sample=${snapshot?.takeLast(120)}",
            seen,
        )
    }

    private fun longPressCenter(): View {
        val view = surface()
        injectLongPress(view, view.width / 2f, view.height / 2f)
        return view
    }

    @假如("^终端显示文本$")
    fun terminalDisplaysText() {
        composeRuleHolder.composeRule.waitForSession()
        ensureScreenText()
    }

    @假如("^终端中的文本已被选中$")
    fun textIsSelectedInTerminal() {
        composeRuleHolder.composeRule.waitForSession()
        ensureScreenText()
        longPressCenter()
        val selected =
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                selection().active
            }
        assertNotNull("长按后未进入选择状态", selected)
    }

    @当("^长按字符$")
    fun userLongPressesOnCharacter() {
        longPressCenter()
    }

    @当("^长按空白区域$")
    fun userLongPressesOnEmptyArea() {
        val view = surface()
        // 顶部是提示符与数字行，底部 90% 处为空行，长按此处应出粘贴菜单而非单词选中。
        injectLongPress(view, view.width / 2f, view.height * 0.9f)
    }

    @当("^向前拖动选择手柄$")
    fun userDragsSelectionHandleForward() {
        val view = surface()
        dragBeforeEnd = selection().end
        UxTestUtils.injectDrag(
            view,
            view.width * 0.5f,
            view.height / 2f,
            view.width * 0.75f,
            view.height / 2f,
        )
    }

    @当("^向后拖动选择手柄$")
    fun userDragsSelectionHandleBackward() {
        val view = surface()
        dragBeforeEnd = selection().end
        UxTestUtils.injectDrag(
            view,
            view.width * 0.5f,
            view.height / 2f,
            view.width * 0.25f,
            view.height / 2f,
        )
    }

    @当("^触发复制$")
    fun userTriggersCopy() {
        // 复制会清除选区，先记下所选文本供断言比对。
        val selectedText = selection().selectedText
        assertTrue("复制前应有选中文本", selectedText.isNotEmpty())
        copiedText = selectedText
        // 选择菜单是原生 PopupWindow，不在 Compose 语义树中，用 UiAutomator 点击。
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        assertTrue("选择菜单未出现", device.wait(Until.hasObject(By.text("复制")), 10_000))
        device.findObject(By.text("复制")).click()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @那么("^出现选择手柄$")
    fun selectionHandleAppears() {
        // 手柄是 Surface 侧 TYPE_APPLICATION_SUB_PANEL 覆盖窗（HandleOverlayLayout），
        // 不在 Compose 语义树中，按视图类名用 UiAutomator 等待其出现。
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        assertTrue(
            "选择手柄覆盖窗未出现",
            device.wait(Until.hasObject(By.clazz(HANDLE_OVERLAY_CLASS)), HANDLE_WAIT_TIMEOUT_MS),
        )
    }

    @那么("^单词被选中$")
    fun wordIsSelected() {
        composeRuleHolder.composeRule.waitForIdle()
        val selectedText = selection().selectedText
        assertTrue("应选中单词，实际选中文本为空", selectedText.isNotEmpty())
    }

    @那么("^所选文本已在剪贴板$")
    @SuppressLint("DeprecatedCall") // primaryClip：API 37 未标记废弃，lint 数据滞后
    fun textIsAvailableOnClipboard() {
        val expected = checkNotNull(copiedText) { "复制步骤未记录所选文本" }
        copiedText = null
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val actual = clipboard.primaryClip?.getItemAt(0)?.text?.toString()
        assertTrue("剪贴板内容应为所选文本 $expected，实际 $actual", actual == expected)
    }

    @那么("^粘贴菜单已出现$")
    fun pastePopupAppears() {
        composeRuleHolder.composeRule.waitForIdle()
        // 空白处长按产生纯粘贴选区（SelectionManager.showPastePopup），带悬浮粘贴菜单。
        var pasteOnly = false
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            pasteOnly = (activity as MainActivity).terminalViewModel.state.value.selection.pasteOnly
        }
        assertTrue("应为纯粘贴选区", pasteOnly)
    }

    @那么("^选区扩展到拖动目标$")
    fun selectionExtendsToDragTarget() {
        val before = checkNotNull(dragBeforeEnd) { "拖动步骤未记录拖前选区" }
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                selection().active && positionOf(selection().end) != positionOf(before)
            }
        assertNotNull("向前拖动后选区尾部应移动", settled)
        val after = selection().end
        assertTrue(
            "向前拖动后选区尾部应前移，拖前 $before，拖后 $after",
            after != null && positionOf(after) > positionOf(before),
        )
    }

    @那么("^选区收缩到拖动目标$")
    fun selectionShrinksToDragTarget() {
        val before = checkNotNull(dragBeforeEnd) { "拖动步骤未记录拖前选区" }
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                selection().active && positionOf(selection().end) != positionOf(before)
            }
        assertNotNull("向后拖动后选区尾部应移动", settled)
        val after = selection().end
        assertTrue(
            "向后拖动后选区尾部应后移，拖前 $before，拖后 $after",
            after != null && positionOf(after) < positionOf(before),
        )
    }

    private fun positionOf(anchor: SelectionAnchor?): Long {
        if (anchor == null) return -1
        return anchor.row * 10_000L + anchor.col
    }
}
