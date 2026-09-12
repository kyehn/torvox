package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodes
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.espresso.Espresso
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import javax.inject.Inject
import org.junit.Assert.assertTrue
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.waitForSession

class ThemeSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
  // 跨场景共享的胶水实例会常驻，字段只在同一场景的当/那么之间传递。
  private var selectedThemeName: String? = null

  private fun currentThemeNames(): Triple<String, String, String> {
    var names = Triple("", "", "")
    composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
      val settings = activity.terminalViewModel.settings.value
      names = Triple(settings.themeName, settings.dayThemeName, settings.nightThemeName)
    }
    return names
  }

  @当("^用户选择另一个主题$")
  fun userSelectsDifferentTheme() {
    val before = currentThemeNames()
    // 选一个当前未使用的内置主题，保证点下去一定是“切换”而非原地确认。
    val target =
        listOf("Catppuccin Mocha", "Dracula Plus").firstOrNull {
          it != before.first && it != before.second && it != before.third
        } ?: "Catppuccin Mocha"
    selectedThemeName = target
    composeRuleHolder.composeRule
        .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = true)
        .performScrollToNode(hasTestTag("ThemeSelector"))
    composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
      try {
        composeRuleHolder.composeRule
            .onAllNodes(hasText(target), useUnmergedTree = true)[0]
            .assertIsDisplayed()
        true
      } catch (_: AssertionError) {
        false
      }
    }
    composeRuleHolder.composeRule
        .onAllNodes(hasText(target), useUnmergedTree = true)[0]
        .performClick()
    composeRuleHolder.composeRule.waitForIdle()
  }

  @那么("^终端主题变为所选主题$")
  fun terminalThemeUpdates() {
    val target = checkNotNull(selectedThemeName) { "主题切换步骤未记录所选主题" }
    selectedThemeName = null
    val after = currentThemeNames()
    assertTrue(
        "点击 $target 后主题状态应变为该主题，实际 $after",
        after.first == target || after.second == target || after.third == target,
    )
    Espresso.pressBack()
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
  }

  @那么("^终端应用的是有效内置主题$")
  fun appliedThemeIsValidBuiltIn() {
    composeRuleHolder.composeRule.waitForSession()
    val validNames = BuiltInThemes.all.map { it.name }.toSet()
    val current = currentThemeNames()
    assertTrue(
        "当前主题应为有效内置主题，实际 $current",
        current.first in validNames && current.second in validNames && current.third in validNames,
    )
    composeRuleHolder.composeRule
        .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
        .assertIsDisplayed()
  }
}
