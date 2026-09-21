package terminal.emulator.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import terminal.emulator.SessionInfo

/**
 * 抽屉窄屏不断言：最小 240.dp 宽度下四个操作按钮必须直接可见
 * （两行两列网格，无需横向滚动），不溢出崩溃、不截断标签。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SessionDrawerNarrowTest {

    @get:Rule val composeRule = createComposeRule()

    private fun setDrawer() {
        composeRule.setContent {
            MaterialTheme {
                Box(Modifier.width(240.dp)) {
                    SessionDrawer(
                        sessions = emptyList(),
                        activeSessionId = 0L,
                        onSwitchSession = {},
                        onCloseSession = {},
                        onAddSession = {},
                        onRefreshSessions = {},
                        onSettings = {},
                        onSearch = {},
                        onKeyboardToggle = {},
                        onResetTerminal = {},
                        onClose = {},
                    )
                }
            }
        }
    }

    @Test
    fun narrow_drawer_keeps_all_action_buttons() {
        setDrawer()
        for (tag in listOf("SearchButton", "KeyboardToggle", "ResetTerminalButton", "SettingsButton")) {
            val count =
                composeRule.onAllNodes(hasTestTag(tag), useUnmergedTree = true)
                    .fetchSemanticsNodes().size
            assertTrue("按钮必须存在: $tag", count == 1)
        }
    }

    @Test
    fun narrow_drawer_last_button_visible_without_scrolling() {
        setDrawer()
        // 两行两列网格：设置按钮必须直接可见，无需滚动。
        composeRule.onNodeWithTag("SettingsButton").assertIsDisplayed()
    }

    @Test
    fun narrow_drawer_session_item_renders() {
        composeRule.setContent {
            MaterialTheme {
                Box(Modifier.width(240.dp)) {
                    SessionDrawer(
                        sessions =
                        listOf(
                            SessionInfo(
                                id = 1L,
                                title = "1",
                                directory = "/data/data/com.termux/files/home",
                            ),
                        ),
                        activeSessionId = 1L,
                        onSwitchSession = {},
                        onCloseSession = {},
                        onAddSession = {},
                        onRefreshSessions = {},
                        onSettings = {},
                        onSearch = {},
                        onKeyboardToggle = {},
                        onResetTerminal = {},
                        onClose = {},
                    )
                }
            }
        }
        val items =
            composeRule.onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        assertTrue("会话项必须存在", items.size == 1)
    }
}
