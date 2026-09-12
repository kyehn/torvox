package terminal.emulator.cucumber.steps

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipe
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import javax.inject.Inject
import org.junit.Assert.assertTrue
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openSettings

class SettingsSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
  // 滑块槽位 8..48sp，手指停在 80% 处时期望值约 40sp（步进吸附后 39.67），
  // 触摸抖动与滑块内边距会导致小偏差，容差 2sp。
  private companion object {
    const val EXPECTED_FONT_SIZE = 39.67f
    const val FONT_SIZE_TOLERANCE = 2.0f
  }

  @当("^打开设置界面$")
  fun userOpensSettingsScreen() {
    composeRuleHolder.composeRule.openSettings()
  }

  @那么("^主题选择器与字号滑块已显示$")
  fun settingsSectionsDisplayed() {
    composeRuleHolder.composeRule
        .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
        .assertIsDisplayed()
    composeRuleHolder.composeRule
        .onNodeWithTag("FontSizeSlider", useUnmergedTree = true)
        .assertIsDisplayed()
  }

  @那么("^字号滑块存在$")
  fun fontSizeSliderExists() {
    composeRuleHolder.composeRule
        .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
        .assertIsDisplayed()
    composeRuleHolder.composeRule
        .onNodeWithTag("FontSizeSlider", useUnmergedTree = true)
        .assertIsDisplayed()
  }

  @当("^调节滑块$")
  fun sliderIsAdjusted() {
    composeRuleHolder.composeRule
        .onNodeWithTag("FontSizeSlider", useUnmergedTree = true)
        .performTouchInput {
          swipe(
              Offset(width * 0.2f, height * 0.5f),
              Offset(width * 0.8f, height * 0.5f),
          )
        }
    composeRuleHolder.composeRule.waitForIdle()
  }

  @那么("^终端字号变为预期值$")
  fun terminalFontSizeReachesExpected() {
    composeRuleHolder.composeRule
        .onNodeWithTag("SettingsBackButton", useUnmergedTree = true)
        .performClick()
    composeRuleHolder.composeRule.waitForIdle()
    composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
      composeRuleHolder.composeRule
          .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
          .fetchSemanticsNodes()
          .isNotEmpty()
    }
    composeRuleHolder.composeRule
        .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
        .assertIsDisplayed()
    // 真断言：手势必须真正驱动设置值，而不只是界面仍在。
    var fontSize = 0f
    composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
      fontSize = activity.terminalViewModel.settings.value.fontSize
    }
    assertTrue(
        "滑块手势后字号应为约 $EXPECTED_FONT_SIZE，实际 $fontSize",
        kotlin.math.abs(fontSize - EXPECTED_FONT_SIZE) <= FONT_SIZE_TOLERANCE,
    )
  }
}
