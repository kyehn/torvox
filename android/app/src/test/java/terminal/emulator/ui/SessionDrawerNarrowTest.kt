package terminal.emulator.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import terminal.emulator.SessionInfo
import kotlin.math.abs

/**
 * 抽屉窄屏不断言：最小 240.dp 宽度下四个操作按钮必须直接可见
 * （两行两列网格，无需横向滚动），不溢出崩溃、不截断标签。
 *
 * 同时钉住 `DESIGN.md` 侧边面板节的布局口径：动作按「重置终端 文本搜索 /
 * 显示、隐藏输入法 设置」两行两列排列，「新建会话」位于会话列表底部。
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

    private fun setDrawerWithSession() {
        composeRule.setContent {
            MaterialTheme {
                Box(Modifier.width(240.dp)) {
                    SessionDrawer(
                        sessions =
                        listOf(
                            SessionInfo(
                                sessionId = 1L,
                                title = "VT_TITLE",
                            ),
                        ),
                        activeSessionId = 1L,
                        onSwitchSession = {},
                        onCloseSession = {},
                        onAddSession = {},
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

    /** 节点在根坐标里的纵向范围（上沿、下沿），单位像素。 */
    private fun verticalRange(tag: String): Pair<Float, Float> {
        val node = composeRule.onNodeWithTag(tag).fetchSemanticsNode()
        val top = node.positionInRoot.y
        return top to (top + node.size.height)
    }

    /** 同行判定：布局按像素取整，容差 1 像素。 */
    private fun assertSameRow(firstTop: Float, secondTop: Float) {
        assertTrue("同一行的两个动作必须等高（上沿 $firstTop 与 $secondTop）", abs(firstTop - secondTop) < 1f)
    }

    @Test
    fun narrow_drawer_keeps_all_action_buttons() {
        setDrawer()
        for (tag in listOf("SearchButton", "KeyboardToggle", "ResetTerminalButton", "SettingsButton")) {
            val count =
                composeRule
                    .onAllNodes(hasTestTag(tag), useUnmergedTree = true)
                    .fetchSemanticsNodes()
                    .size
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
        setDrawerWithSession()
        val items =
            composeRule
                .onAllNodes(hasTestTag("SessionItem"), useUnmergedTree = true)
                .fetchSemanticsNodes()
        assertTrue("会话项必须存在", items.size == 1)
        // 序号与终端标题同行（termux 无会话名时的渲染，DESIGN 示例 `[1] ~`）。
        composeRule.onNodeWithText("[1] VT_TITLE").assertIsDisplayed()
    }

    @Test
    fun drawer_actions_follow_spec_layout() {
        setDrawer()
        val resetTop = verticalRange("ResetTerminalButton").first
        val searchTop = verticalRange("SearchButton").first
        val keyboardTop = verticalRange("KeyboardToggle").first
        val settingsTop = verticalRange("SettingsButton").first
        assertSameRow(resetTop, searchTop)
        assertSameRow(keyboardTop, settingsTop)
        assertTrue("第一行必须位于第二行之上（重置终端 $resetTop，键盘 $keyboardTop）", resetTop < keyboardTop)
    }

    @Test
    fun add_session_button_sits_below_session_list() {
        setDrawerWithSession()
        val itemBottom = verticalRange("SessionItem").second
        val addTop = verticalRange("AddSessionButton").first
        assertTrue("新建会话必须位于会话列表下方（会话项底边 $itemBottom，按钮顶边 $addTop）", addTop >= itemBottom)
    }
}
