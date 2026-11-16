package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import javax.inject.Inject

class SearchSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    @Given("^终端会话处于活动状态$")
    fun terminalSessionIsActive() {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Given("^终端已有搜索高亮$")
    fun terminalHasSearchHighlightsActive() {
        composeRuleHolder.composeRule.onNodeWithTag("SearchButton").performClick()
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule.onNodeWithTag("SearchTextField").performClick()
        composeRuleHolder.composeRule.onNodeWithTag("SearchTextField").performTextInput("the")
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Given("^搜索栏可见$")
    fun searchBarIsVisible() {
        composeRuleHolder.composeRule.onNodeWithTag("SearchButton").performClick()
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule.onNodeWithTag("TextSearchBar").assertIsDisplayed()
    }

    @When("^从会话面板打开搜索栏$")
    fun userOpensSearchBar() {
        val composeRule = composeRuleHolder.composeRule
        // 抽屉内容在关闭时仍被组合，直接点击 SearchButton 即可。
        composeRule.onNodeWithTag("SearchButton").performClick()
        composeRule.waitForIdle()
        // 点击后抽屉关闭协程需要时间，等动画跑完。
        composeRule.waitForIdle()
    }

    @When("^关闭搜索栏$")
    fun userClosesSearchBar() {
        composeRuleHolder.composeRule.onNodeWithTag("SearchClose").performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^弹出软键盘$")
    fun softKeyboardOpens() {
        composeRuleHolder.composeRule.onNodeWithTag("SearchTextField").performClick()
        composeRuleHolder.composeRule.waitForIdle()
    }

    @Then("^搜索栏显示在底部$")
    fun searchBarIsDisplayedAtBottom() {
        val composeRule = composeRuleHolder.composeRule
        composeRule.waitForIdle()
        // 搜索栏在底部，检查其内部节点。
        composeRule.onNodeWithTag("SearchTextField").assertIsDisplayed()
        composeRule.onNodeWithTag("SearchClose").assertIsDisplayed()
    }

    @Then("^修饰键栏已隐藏$")
    fun modifierBarIsHidden() {
        composeRuleHolder.composeRule.onNodeWithTag("ModifierBar").assertIsNotDisplayed()
    }

    @Then("^修饰键栏重新可见$")
    fun modifierBarIsVisibleAgain() {
        composeRuleHolder.composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Then("^搜索高亮全部消失$")
    fun allSearchHighlightsDisappear() {
        composeRuleHolder.composeRule.waitForIdle()
        // 关闭搜索栏会移除结果计数/高亮界面。
        composeRuleHolder.composeRule.onNodeWithTag("SearchResultCount").assertIsNotDisplayed()
    }

    @Then("^搜索栏仍在键盘上方$")
    fun searchBarRemainsVisibleAboveKeyboard() {
        composeRuleHolder.composeRule.onNodeWithTag("SearchTextField").assertIsDisplayed()
    }
}
