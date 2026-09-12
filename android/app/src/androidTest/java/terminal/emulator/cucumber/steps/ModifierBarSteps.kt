package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import javax.inject.Inject
import terminal.emulator.cucumber.ComposeRuleHolder

class ModifierBarSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
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
    rule.onNodeWithTag("Key_CTRL").performClick()
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
