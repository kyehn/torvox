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
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
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
 * separation in tests" (see docs/specification/TESTING.md §Instrumented 方法论).
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
                    onSubmit = {},
                    onKey = { _, _ -> false },
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

    private fun setClickCountingBar(onKey: (Int, Int) -> Boolean) {
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(onSubmit = {}, onKey = onKey)
            }
        }
    }

    @Test
    fun `plain extra key fires on release within slop`() {
        // spec modifier-bar-interaction 松手确认：单击在抬手时触发一次；
        // 任一阶段滑出触摸容差即取消，底部上滑不再误触按键。
        var clicks = 0
        setClickCountingBar { _, _ ->
            clicks++
            true
        }
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
        setClickCountingBar { _, _ ->
            clicks++
            true
        }
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
        // Termux 对等接线：点击必须使 CTRL 进入 armed（selected）态，
        // 对应设备场景「修饰键可以切换状态」曾回归为 Selected=false。
        var ctrl by mutableStateOf(ModifierState.Off)
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onSubmit = {},
                    onKey = { _, _ -> false },
                    ctrlState = ctrl,
                    onToggleCtrl = { ctrl = ctrl.next() },
                )
            }
        }
        composeRule.onNodeWithTag("Key_CTRL").performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithTag("Key_CTRL").assertIsSelected()
    }

    @Test
    fun `plain extra keys route through the encoder with the android keycode`() {
        // 键栏不再自带转义序列表（ToolbarKey.sequence 已删）：每次点击只允许
        // 上送 Android 键码，字节由原生编码器产出。
        val sent = mutableListOf<Pair<Int, Int>>()
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(onSubmit = {}, onKey = { keyCode, modifiers ->
                    sent.add(keyCode to modifiers)
                    true
                })
            }
        }
        composeRule.onNodeWithTag("Key_PGUP").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(
            listOf(android.view.KeyEvent.KEYCODE_PAGE_UP to 0),
            sent,
        )
    }

    @Test
    fun `ctrl sticky key routes upstream modifier bits to the encoder`() {
        // 粘滞 CTRL 点亮后点 PGUP：上送的必须是上游 key.Mods 的 CTRL 位（2），
        // 且 Once 态被消费。
        val sent = mutableListOf<Pair<Int, Int>>()
        var consumed = 0
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onSubmit = {},
                    onKey = { keyCode, modifiers ->
                        sent.add(keyCode to modifiers)
                        true
                    },
                    onConsumeModifiers = { consumed++ },
                    ctrlState = ModifierState.Once,
                )
            }
        }
        composeRule.onNodeWithTag("Key_PGUP").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(
            listOf(android.view.KeyEvent.KEYCODE_PAGE_UP to terminal.emulator.input.KeyModifiers.CTRL),
            sent,
        )
        org.junit.Assert.assertEquals(1, consumed)
    }

    @Test
    fun `keys without a keycode are not sent`() {
        // DRAWER/SCROLL 必须走自己的回调，不得进入按键编码通道。
        val sent = mutableListOf<Pair<Int, Int>>()
        var drawerOpens = 0
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onSubmit = {},
                    onKey = { keyCode, modifiers ->
                        sent.add(keyCode to modifiers)
                        true
                    },
                    onDrawerClick = { drawerOpens++ },
                )
            }
        }
        composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertTrue("DRAWER 不得进入按键编码通道", sent.isEmpty())
        org.junit.Assert.assertEquals(1, drawerOpens)
    }

    @Test
    fun `swipe left enters text input page and swipe right returns`() {
        // DESIGN.md 修饰键栏节「支持左右滑动在按键页与文本输入页之间切换」：分页器两页
        // （按键页 → 文本输入页），此前两向滑动都没有覆盖。
        setModifierBar()
        composeRule.onNodeWithTag("TextInputPage").assertDoesNotExist()
        composeRule.onNodeWithTag("ModifierBarPager").performTouchInput { swipeLeft() }
        composeRule.waitForIdle()
        composeRule.onNodeWithTag("TextInputPage").assertIsDisplayed()
        composeRule.onNodeWithTag("ModifierBarPager").performTouchInput { swipeRight() }
        composeRule.waitForIdle()
        composeRule.onNodeWithTag("TextInputPage").assertDoesNotExist()
        composeRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
    }

    @Test
    fun `drawer long press pastes clipboard`() {
        // Termux `popup: 'PASTE'`：DRAWER 长按必须粘贴，轻点仍开抽屉。
        var pastes = 0
        var drawerOpens = 0
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onSubmit = {},
                    onKey = { _, _ -> false },
                    onDrawerClick = { drawerOpens++ },
                    onPasteClick = { pastes++ },
                )
            }
        }
        composeRule.onNodeWithTag("Key_DRAWER").performTouchInput {
            down(center)
            advanceEventTime(LONG_PRESS_HOLD_MS)
            up()
        }
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals("长按必须粘贴", 1, pastes)
        org.junit.Assert.assertEquals("长按不得打开抽屉", 0, drawerOpens)
    }

    @Test
    fun `drawer tap opens drawer without pasting`() {
        var pastes = 0
        var drawerOpens = 0
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onSubmit = {},
                    onKey = { _, _ -> false },
                    onDrawerClick = { drawerOpens++ },
                    onPasteClick = { pastes++ },
                )
            }
        }
        composeRule.onNodeWithTag("Key_DRAWER").performClick()
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals("轻点必须打开抽屉", 1, drawerOpens)
        org.junit.Assert.assertEquals("轻点不得粘贴", 0, pastes)
    }

    private companion object {
        /** 长按阈值（系统值，默认 500ms）之上留出余量。 */
        const val LONG_PRESS_HOLD_MS = 700L
    }
}
