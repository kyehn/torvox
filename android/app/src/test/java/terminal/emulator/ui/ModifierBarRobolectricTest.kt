package terminal.emulator.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import kotlinx.collections.immutable.persistentListOf
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import terminal.emulator.input.ModifierState
import terminal.emulator.input.next

/**
 * JVM (Robolectric) Compose tests for [ModifierBar] — proof that pure-UI semantics run WITHOUT an
 * emulator. ModifierBar has no JNI dependency, so it is the reference case for "backend/frontend
 * separation in tests" (see docs/standards/TESTING.md §Instrumented 方法论).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class ModifierBarRobolectricTest {

    @get:Rule val composeRule = createComposeRule()

    private fun setModifierBar() {
        composeRule.setContent {
            MaterialTheme {
                var ctrl by remember { mutableStateOf(ModifierState.Off) }
                var alt by remember { mutableStateOf(ModifierState.Off) }
                ModifierBar(
                    onKeyClick = {},
                    ctrlState = ctrl,
                    altState = alt,
                    onToggleCtrl = { ctrl = ctrl.next() },
                    onToggleAlt = { alt = alt.next() },
                )
            }
        }
    }

    @Test
    fun `ctrl key visible and toggles selection state`() {
        setModifierBar()
        composeRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
        composeRule.onNodeWithTag("Key_CTRL").performClick()
        composeRule.onNodeWithTag("Key_CTRL").assertIsSelected()
    }

    @Test
    fun `arrow keys and common keys are rendered`() {
        setModifierBar()
        composeRule.onNodeWithTag("Key_\u2191").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_HOME").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_TAB").assertIsDisplayed()
    }

    private fun setClickCountingBar(onKeyClick: (String) -> Unit) {
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = onKeyClick)
            }
        }
    }

    @Test
    fun `plain extra key fires on release within slop`() {
        // spec modifier-bar-interaction 松手确认：单击在抬手时触发一次；
        // 任一阶段滑出触摸容差即取消，底部上滑不再误触按键。
        var clicks = 0
        setClickCountingBar { clicks++ }
        composeRule.onNodeWithTag("Key_ESC").performTouchInput {
            down(center)
            up()
        }
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(
            "tap must fire once on release",
            1,
            clicks,
        )
    }

    @Test
    fun `slide off key cancels the tap without firing`() {
        // 底部上滑误触回归：按下后滑出按键再松手，不得触发。
        var clicks = 0
        setClickCountingBar { clicks++ }
        composeRule.onNodeWithTag("Key_ESC").performTouchInput {
            down(center)
            moveBy(Offset(0f, -300f))
            up()
        }
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(
            "swipe off the key must not fire",
            0,
            clicks,
        )
    }

    @Test
    fun `configurable ctrl key toggles selected on tap`() {
        // App path (toolbarLayout != null) with Termux-parity wiring: a tap
        // must arm CTRL (selected), mirroring the device scenario
        // 修饰键可以切换状态 which regressed to Selected=false.
        var ctrl by mutableStateOf(ModifierState.Off)
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = {},
                    ctrlState = ctrl,
                    onToggleCtrl = { ctrl = ctrl.next() },
                    toolbarLayout = persistentListOf(ToolbarItem.Default(ToolbarKey.CTRL)),
                )
            }
        }
        composeRule.onNodeWithTag("Key_CTRL").performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithTag("Key_CTRL").assertIsSelected()
    }

    @Test
    fun `arrow sends CSI in normal cursor mode`() {
        val sent = mutableListOf<String>()
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = { sent.add(it) })
            }
        }
        composeRule.onNodeWithTag("Key_\u2192").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(listOf("\u001b[C"), sent)
    }

    @Test
    fun `arrow sends SS3 in application cursor mode`() {
        // DECCKM 回归：vim/less 等应用光标模式下辅助键栏方向键须发 SS3，
        // 与物理键盘路径（TerminalInputEncoder.arrowSequence）一致。
        val sent = mutableListOf<String>()
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = { sent.add(it) }, isAppCursorMode = { true })
            }
        }
        composeRule.onNodeWithTag("Key_\u2192").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(listOf("\u001bOC"), sent)
    }

    @Test
    fun `configurable arrow sends CSI in normal cursor mode`() {
        // 生产路径恒走可配置键栏：方向键须同样跟随光标模式，普通模式发 CSI。
        val sent = mutableListOf<String>()
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { sent.add(it) },
                    toolbarLayout = persistentListOf(ToolbarItem.Default(ToolbarKey.ARROW_UP)),
                )
            }
        }
        composeRule.onNodeWithTag("Key_↑").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(listOf("\u001b[A"), sent)
    }

    @Test
    fun `configurable arrow sends SS3 in application cursor mode`() {
        // 应用光标模式（less/vim）下可配置键栏方向键须发 SS3，否则分页器无反应。
        val sent = mutableListOf<String>()
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { sent.add(it) },
                    toolbarLayout = persistentListOf(ToolbarItem.Default(ToolbarKey.ARROW_UP)),
                    isAppCursorMode = { true },
                )
            }
        }
        composeRule.onNodeWithTag("Key_↑").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(listOf("\u001bOA"), sent)
    }
}
