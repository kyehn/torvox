package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.zh_cn.假如
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.getBridge
import terminal.emulator.probeAssertion
import javax.inject.Inject

class ModifierBarSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    companion object {
        // 跨场景共享 activity，CTRL 可能被之前场景留在 armed 态：归一到关闭态
        // 再断言切换，上限内仍未关闭则大声失败（不断言掩盖真坏）。
        private const val TOGGLE_NORMALIZE_MAX_TAPS = 3
    }

    @假如("^CTRL 键处于关闭态$")
    fun ctrlKeyStartsOff() {
        val rule = composeRuleHolder.composeRule
        repeat(TOGGLE_NORMALIZE_MAX_TAPS) {
            if (probeAssertion { rule.onNodeWithTag("Key_CTRL").assertIsNotSelected() }) return
            rule.onNodeWithTag("Key_CTRL").performClick()
            rule.waitForIdle()
        }
        rule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
    }

    @那么("^修饰键栏显示 ESC TAB CTRL ALT HOME END PGUP PGDN 按键$")
    fun modifierBarShowsAllKeys() {
        composeRuleHolder.composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_TAB").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_HOME").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_END").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_PGUP").assertIsDisplayed()
        composeRuleHolder.composeRule.onNodeWithTag("Key_PGDN").assertIsDisplayed()
    }

    @当("^轻触 CTRL 键$")
    fun ctrlKeyIsTapped() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_CTRL").performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @当("^双击 CTRL 键$")
    fun ctrlKeyIsTappedTwice() {
        val rule = composeRuleHolder.composeRule
        // 双击 = 两次点按：第一次选中，第二次取消选中。
        // 两次 performClick 之间等选中态落定（waitForIdle 不等重组），
        // 落定后直接点第二次：产品已修偷 Once 的 bug，此处不再重试。
        // 双击前先让 shell 吐出可观测输出：若 PTY 桥尚未就绪（会话孵化中），
        // 先等就绪再点——之前失败的真正面目可能是桥未就绪时点击被吞，
        // 而非手势竞态。用输出存在性做可观测门禁，大声失败。
        val bridge = rule.getBridge() ?: throw AssertionError("拿不到终端桥")
        var echoed = false
        val deadline = System.currentTimeMillis() + 15000
        while (!echoed && System.currentTimeMillis() < deadline) {
            bridge.writeToPty("echo CTRL_PROBE\n".toByteArray(Charsets.UTF_8))
            Thread.sleep(500)
            echoed = bridge.getTerminalText()?.contains("CTRL_PROBE") == true
        }
        check(echoed) { "shell 无回显，桥未就绪" }
        rule.onNodeWithTag("Key_CTRL").performClick()
        rule.waitUntil(timeoutMillis = 5000) {
            probeAssertion { rule.onNodeWithTag("Key_CTRL").assertIsSelected() }
        }
        rule.onNodeWithTag("Key_CTRL").performClick()
        rule.waitForIdle()
    }

    @那么("^CTRL 键呈选中态$")
    fun ctrlKeyTogglesAppearance() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_CTRL").assertIsSelected()
    }

    @那么("^CTRL 键恢复默认态$")
    fun ctrlKeyReturnsToDefaultAppearance() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
    }
}
