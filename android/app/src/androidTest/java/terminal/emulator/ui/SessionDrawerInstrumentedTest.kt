package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.R
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.getBridge
import terminal.emulator.openDrawer
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 会话抽屉端到端（对标 sylirre TerminalUiTest.newTabCreatesAndSwitchesSessions /
 * closingActiveTabSwitchesToRemaining）：新建会话 → 切走切回（网格内容跟随会话）→
 * 关闭当前会话回落到剩余会话。切换走 UI（抽屉项点击），内容隔离走网格断言。
 */
@RunWith(JUnit4::class)
class SessionDrawerInstrumentedTest {
    companion object {
        private const val STATE_TIMEOUT_MS = 10_000L
        private const val GRID_TIMEOUT_MS = 15_000L
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun currentText(): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    private fun activeSessionId(): Long {
        var id = -1L
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            id = composeTestRule.activity.terminalViewModel.state.value.activeSessionId
        }
        return id
    }

    private fun sessionCount(): Int {
        var count = -1
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            count = composeTestRule.activity.terminalViewModel.state.value.sessions.size
        }
        return count
    }

    @Test
    fun addSwitchAndCloseSession() {
        val idA = activeSessionId()
        assertTrue("初始必须有活跃会话", idA > 0)
        val countBefore = sessionCount()

        // 新建会话 B：经抽屉真实点击，B 成为活跃会话。
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        val switchedToB =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                sessionCount() == countBefore + 1 && activeSessionId() != idA
            }
        assertNotNull("新建后会话数+1 且切到新会话", switchedToB)
        val idB = activeSessionId()

        // 在 B 网格直写标记（经 parser，不依赖 shell 就绪）。
        val markerB = "SESSB${System.currentTimeMillis() % 100000}"
        assertTrue("标记送显失败", bridge().feedTerminal(markerB.toByteArray(Charsets.UTF_8)))
        val markerSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.contains(markerB) == true
            }
        assertNotNull("B 网格必须显示标记", markerSeen)

        // 切回 A：B 的标记必须消失（可见网格跟随会话）。
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithText("会话 1").performClick()
        val backToA =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                activeSessionId() == idA
            }
        assertNotNull("必须切回首个会话", backToA)
        val markerGone =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.contains(markerB) == false
            }
        assertNotNull("切回 A 后 B 的标记必须不可见", markerGone)

        // 切回 B：标记重现。
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithText("会话 2").performClick()
        val backToB =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                activeSessionId() == idB
            }
        assertNotNull("必须切回第二个会话", backToB)
        val markerBack =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.contains(markerB) == true
            }
        assertNotNull("切回 B 后标记必须重现", markerBack)

        // 关闭当前会话 B：回落到 A，会话数恢复。
        composeTestRule.openDrawer()
        val closeDescription = composeTestRule.activity.getString(R.string.cd_close_session)
        composeTestRule.onAllNodesWithContentDescription(closeDescription)[1].performClick()
        val closedBack =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                sessionCount() == countBefore && activeSessionId() == idA
            }
        assertNotNull("关闭 B 后必须回落到 A 且数量恢复", closedBack)
    }
}
