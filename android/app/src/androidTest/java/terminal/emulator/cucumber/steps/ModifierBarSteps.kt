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
        // performClick 在 cucumber 规则下第二次点击会被手势残留吞掉，
        // 改用 UiDevice 点击视图中心坐标（与 SelectionSteps 的
        // injectLongPress 同一底层路径，绕过 compose 手势协程）。
        rule.onNodeWithTag("Key_CTRL").performClick()
        rule.waitUntil(timeoutMillis = 5000) {
            probeAssertion { rule.onNodeWithTag("Key_CTRL").assertIsSelected() }
        }
        // compose 语义树的 boundsInRoot 是逻辑坐标，直接喂给 UiDevice
        // 会点偏（实测 uiautomator 坐标系下 CTRL 在 [155,2242][310,2337]）。
        // 改用 UiSelector 按 content-desc 定位后点击，坐标由系统解析。
        val device =
            androidx.test.platform.app.InstrumentationRegistry
                .getInstrumentation()
                .let { androidx.test.uiautomator.UiDevice.getInstance(it) }
        val ctrlButton =
            device.findObject(
                androidx.test.uiautomator.By.desc("Ctrl 切换"),
            )
        check(ctrlButton != null) { "找不到 Ctrl 切换按钮" }
        ctrlButton.click()
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
