package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.rule.GrantPermissionRule
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.sessionCount

/**
 * 会话创建：入口可见可点、新建后终端仍在、新建耗时在预算内、会话数确实 +1。
 *
 * 每例新建的会话一律收尾关闭：`:app:connectedDebugAndroidTest` 全部用例同进程，
 * 会话跨用例存活，不关即把会话表越堆越长，抽屉列表随之变长、后续按序号定位的
 * 用例全部漂移。
 *
 * 断言一律用 JUnit 而非 Kotlin `assert`：ART 默认不带 `-ea`，`assert` 在
 * 仪器化进程里恒为静默空操作——原实现的两处 `assert` 实际什么都没断言。
 */
class SessionCreationInstrumentedTest : TerminalLogcatTest() {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private var sessionsBefore = 0

    @Before
    fun setUp() {
        sessionsBefore = composeTestRule.sessionCount()
    }

    @After
    fun tearDown() {
        val closed =
            UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
                if (composeTestRule.sessionCount() <= sessionsBefore) {
                    true
                } else {
                    composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                        val viewModel = activity.terminalViewModel
                        val open = viewModel.runtime.state.value.sessionIds
                        if (open.size > sessionsBefore) {
                            viewModel.closeSession(open.max())
                        }
                    }
                    false
                }
            }
        assertTrue("新建的会话必须全部关闭（残留 ${composeTestRule.sessionCount()} > $sessionsBefore）", closed != null)
    }

    /**
     * 经抽屉新增一个会话并等它真正出现在会话表里。
     *
     * 会话创建是异步的：点「新增」后要经运行时孵化、首帧重排才落到会话表，
     * `waitForIdle` 只排干 Compose 重组队列，不等这条链路。原先按点击后立即
     * 读会话数，读到的恒是旧值。
     */
    private fun addSessionViaDrawer(expectedTotal: Int) {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        val created =
            UxTestUtils.pollUntilTrue(timeoutMs = SESSION_CREATION_BUDGET_MS, intervalMs = 200) {
                composeTestRule.sessionCount() >= expectedTotal
            }
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        assertTrue("新建会话必须在 ${SESSION_CREATION_BUDGET_MS}ms 内出现（实际 ${composeTestRule.sessionCount()}）", created != null)
    }

    @Test
    fun add_session_button_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }

    @Test
    fun add_session_button_clickable() {
        addSessionViaDrawer(sessionsBefore + 1)
        assertTrue("新建后会话数必须 +1", composeTestRule.sessionCount() == sessionsBefore + 1)
    }

    @Test
    fun add_session_completes_within_timeout() {
        val startTime = System.currentTimeMillis()
        addSessionViaDrawer(sessionsBefore + 1)
        val elapsed = System.currentTimeMillis() - startTime
        assertTrue("会话创建耗时 ${elapsed}ms，超出 10000ms 预算", elapsed < SESSION_CREATION_BUDGET_MS)
    }

    @Test
    fun add_second_session_does_not_crash() {
        addSessionViaDrawer(sessionsBefore + 1)
        addSessionViaDrawer(sessionsBefore + 2)
        assertTrue(
            "连续新建两次后会话数应为 ${sessionsBefore + 2}，实际 ${composeTestRule.sessionCount()}",
            composeTestRule.sessionCount() == sessionsBefore + 2,
        )
    }

    @Test
    fun session_drawer_shows_multiple_sessions() {
        addSessionViaDrawer(sessionsBefore + 1)
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        // 抽屉是 LazyColumn：组合晚于点击，逐帧轮询到两个会话项都进树。
        val listed =
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 200) {
                composeTestRule.onAllNodesWithTag("SessionItem").fetchSemanticsNodes().size >= 2
            }
        assertTrue(
            "抽屉必须列出至少两个会话（会话数=${composeTestRule.sessionCount()}，抽屉项=" +
                "${composeTestRule.onAllNodesWithTag("SessionItem").fetchSemanticsNodes().size}）",
            listed != null,
        )
    }

    private companion object {
        /** 会话创建耗时预算（毫秒）。 */
        const val SESSION_CREATION_BUDGET_MS = 10_000L
    }
}
