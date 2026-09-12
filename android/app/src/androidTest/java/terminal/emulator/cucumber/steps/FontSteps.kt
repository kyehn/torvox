package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.test.platform.app.InstrumentationRegistry
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import java.io.File
import javax.inject.Inject
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openSettings

class FontSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
  @当("^打开设置$")
  fun userOpensSettings() {
    composeRuleHolder.composeRule.openSettings()
  }

  @当("^更改字体$")
  fun changesFontFamily() {
    composeRuleHolder.composeRule
        .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
        .assertIsDisplayed()
    // 字体设置项在小屏设备上位于首屏下方，必须先滚动到标题再断言，
    // 否则 onNodeWithText 会因节点不在视口内而断言失败。
    composeRuleHolder.composeRule
        .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = true)
        .performScrollToNode(hasText("字体"))
    composeRuleHolder.composeRule.onNodeWithText("字体").performScrollTo()
    composeRuleHolder.composeRule.onNodeWithText("字体").assertIsDisplayed()
    // 三个槽位共用该标签（常规/粗体/斜体），下标 0 是常规槽位，
    // 正是“更改字体”要点的选择器。
    composeRuleHolder.composeRule
        .onAllNodes(hasTestTag("FontFamilySelector"), useUnmergedTree = true)[0]
        .performScrollTo()
    composeRuleHolder.composeRule
        .onAllNodes(hasTestTag("FontFamilySelector"), useUnmergedTree = true)[0]
        .performClick()
    composeRuleHolder.composeRule.waitForIdle()
  }

  @那么("^字体选择器已打开且无错误$")
  fun fontPickerOpensWithoutError() {
    // 点击选择器应无崩溃地打开字体选择对话框（标题“选择字体”）。
    composeRuleHolder.composeRule.onNodeWithText("选择字体", useUnmergedTree = true).assertIsDisplayed()
  }

  @那么("^字体文件存放在应用私有字体目录$")
  fun fontFilesStoredInPrivateDirectory() {
    val context = InstrumentationRegistry.getInstrumentation().targetContext
    val fontsDir = File(context.filesDir, "fonts")
    // TerminalScreen 先于 TerminalRuntime.start() 可见（私有字体目录是
    // 异步创建的），轮询而非立即断言。
    composeRuleHolder.composeRule.waitUntil(timeoutMillis = 15_000) {
      fontsDir.isDirectory
    }
  }
}
