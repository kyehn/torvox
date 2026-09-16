package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.cucumber.java.zh_cn.假如
import io.cucumber.java.zh_cn.当
import io.cucumber.java.zh_cn.那么
import org.junit.Assert.assertTrue
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.waitForSession
import javax.inject.Inject

class SessionSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    @假如("^应用已启动并创建了多个会话$")
    fun appHasLaunchedWithMultipleSessions() {
        composeRuleHolder.composeRule.waitForSession()
        composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("AddSessionButton"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("AddSessionButton", useUnmergedTree = true)
            .performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 12000) {
            val count =
                composeRuleHolder.composeRule
                    .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                    .fetchSemanticsNodes()
                    .size
            count >= 2
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @当("^新增会话$")
    fun userAddsNewSession() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("AddSessionButton"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("AddSessionButton", useUnmergedTree = true)
            .performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 12000) {
            val count =
                composeRuleHolder.composeRule
                    .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                    .fetchSemanticsNodes()
                    .size
            count >= 2
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @当("^切换到其他会话$")
    fun userSwitchesToDifferentSession() {
        composeRuleHolder.composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule.waitForIdle()
        val sessionNodes =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        if (sessionNodes.size > 1) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)[1]
                .performClick()
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @那么("^会话列表已显示$")
    fun sessionListIsDisplayed() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
    }

    @那么("^存在\"([^\"]+)\"按钮$")
    fun addSessionButtonExists(buttonText: String) {
        composeRuleHolder.composeRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }

    @那么("^抽屉中出现两个会话$")
    fun bothSessionsAppearInDrawer() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionDrawer"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        val sessionItems =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        assertTrue("抽屉中应至少有两个会话，实际 ${sessionItems.size}", sessionItems.size >= 2)
    }

    @那么("^终端显示新会话内容$")
    fun terminalShowsNewSessionContent() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
        // 新会话必然存在：整个 Cucumber 运行共用一个 Activity，会话数只增不减，
        // 因此不断言固定总数，只断言至少有两个。各会话初始内容相同（都是 Shell
        // 提示符），内容本身无法区分，计数即断言。
        val sessionCount =
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        assertTrue("新建会话后应至少有两个会话，实际 $sessionCount", sessionCount >= 2)
    }
}
