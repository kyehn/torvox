package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.rule.GrantPermissionRule
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.R
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.activeSessionId
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.getBridge
import terminal.emulator.openDrawer
import terminal.emulator.sessionCount
import terminal.emulator.sessionIndex
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 会话抽屉端到端（对标 sylirre TerminalUiTest.newTabCreatesAndSwitchesSessions /
 * closingActiveTabSwitchesToRemaining）：新建会话 → 切走切回（网格内容跟随会话）→
 * 关闭当前会话回落到剩余会话。切换走 UI（抽屉项点击），内容隔离走网格断言。
 */
@RunWith(JUnit4::class)
class SessionDrawerInstrumentedTest : TerminalLogcatTest() {
    companion object {
        // 状态轮询上限：CI 软件渲染过载时切换含渲染线程启停，10s 在满载套件下不够。
        private const val STATE_TIMEOUT_MS = 30_000L
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

    /** 共用会话跨全部用例留存：本类建会话并直写标记，不清场即污染后继。 */
    @After
    fun resetSession() = composeTestRule.cleanUpTerminalState()

    /**
     * 某会话网格是否含 [needle]：按会话 id 直读。
     *
     * 不走 `getBridge()`：那读的是**此刻活跃**会话的桥，而「切回 B」断言的是
     * ViewModel 状态已切走，运行时活跃标记何时跟上无从判断——两者错位时读到的是
     * A 的网格，判红原因与切回本身无关。
     */
    private fun sessionContains(sessionId: Long, needle: String): Boolean {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return NativeBridge.getTerminalText(sessionId)?.contains(needle) == true
    }

    private fun currentText(): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    @Test
    fun addSwitchAndCloseSession() {
        // 冷启动会话孵化（首帧网格重算后 attach）晚于桥就绪：轮询等待而非单次读取。
        val initialSession =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.activeSessionId() > 0
            }
        assertNotNull("初始必须有活跃会话", initialSession)
        val idA = composeTestRule.activeSessionId()
        val countBefore = composeTestRule.sessionCount()

        // 新建会话 B：经抽屉真实点击，B 成为活跃会话。
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        val switchedToB =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.sessionCount() == countBefore + 1 && composeTestRule.activeSessionId() != idA
            }
        assertNotNull("新建后会话数+1 且切到新会话", switchedToB)
        val idB = composeTestRule.activeSessionId()

        // 在 B 网格直写标记（经 parser，不依赖 shell 就绪）。必须一次写入并带行结束：
        // B 的 shell 仍在跑，它的提示符（以 `\r` 起头）随时可能抵达；标记不换行时
        // 提示符回到标记所在行首把整行覆盖掉，表现为「切回后标记消失」。
        val markerB = "SESSB${System.currentTimeMillis() % 100000}"
        assertTrue(
            "标记送显失败",
            bridge().feedTerminal("$markerB\r\n".toByteArray(Charsets.UTF_8)),
        )
        val markerSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.contains(markerB) == true
            }
        assertNotNull("B 网格必须显示标记", markerSeen)

        // 切回 A：B 的标记必须消失（可见网格跟随会话）。按 id 定位抽屉位置，不依赖“会话 N”文本。
        composeTestRule.openDrawer()
        val indexA = composeTestRule.sessionIndex(idA)
        assertTrue("抽屉中必须找到首个会话", indexA >= 0)
        composeTestRule.onAllNodesWithTag("SessionItem")[indexA].performClick()
        val backToA =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.activeSessionId() == idA
            }
        assertNotNull("必须切回首个会话", backToA)
        val markerGone =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                currentText()?.contains(markerB) == false
            }
        assertNotNull("切回 A 后 B 的标记必须不可见", markerGone)

        // 切回 B：标记重现。B 追加在末尾，按 id 定位。
        composeTestRule.openDrawer()
        val indexB = composeTestRule.sessionIndex(idB)
        assertTrue("抽屉中必须找到第二个会话", indexB >= 0)
        composeTestRule.onAllNodesWithTag("SessionItem")[indexB].performClick()
        val backToB =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.activeSessionId() == idB
            }
        assertNotNull("必须切回第二个会话", backToB)
        val markerBack =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                sessionContains(idB, markerB)
            }
        assertNotNull(
            "切回 B 后标记必须重现 (会话=$idB 标记=$markerB 实际=${NativeBridge.getTerminalText(idB)?.takeLast(120)})",
            markerBack,
        )

        // 关闭当前会话 B：回落到 A，会话数恢复。B 在末尾，按 id 定位关闭按钮。
        composeTestRule.openDrawer()
        val closeDescription = composeTestRule.activity.getString(R.string.cd_close_session)
        val closeIndex = composeTestRule.sessionIndex(idB)
        assertTrue("抽屉中必须找到待关闭会话", closeIndex >= 0)
        composeTestRule.onAllNodesWithContentDescription(closeDescription)[closeIndex].performClick()
        val closedBack =
            UxTestUtils.pollUntilTrue(timeoutMs = STATE_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.sessionCount() == countBefore && composeTestRule.activeSessionId() == idA
            }
        assertNotNull("关闭 B 后必须回落到 A 且数量恢复", closedBack)
    }
}
