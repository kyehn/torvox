package terminal.emulator.ui

import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsets.Type
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity

class ModifierBarTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun modifier_bar_renders_all_keys() {
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        val expectedTags =
            listOf(
                "Key_ESC",
                "Key_DRAWER",
                "Key_SCROLL",
                "Key_HOME",
                "Key_\u2191",
                "Key_END",
                "Key_PGUP",
                "Key_TAB",
                "Key_CTRL",
                "Key_ALT",
                "Key_\u2190",
                "Key_\u2193",
                "Key_\u2192",
                "Key_PGDN",
            )
        expectedTags.forEach { tag ->
            composeTestRule.onNodeWithTag(tag).assertIsDisplayed()
        }
    }

    @Test
    fun modifier_bar_ctrl_toggle_cycles() {
        // Termux parity: tap toggles one-shot on/off, never locks; lock is
        // long-press only. Armed state is exposed via `selected` semantics.
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsSelected()
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
    }

    @Test
    fun modifier_bar_ctrl_long_press_locks() {
        // Hold past LONG_PRESS_MS (400): the release itself counts even
        // with zero MOVE events, so a plain down/sleep/up is deterministic.
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
        composeTestRule.onNodeWithTag("Key_CTRL").performTouchInput {
            down(center)
            Thread.sleep(650L)
            up()
        }
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsSelected()
        // Tap clears the lock (termux tap-while-locked disarms).
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
    }

    @Test
    fun modifier_bar_alt_toggle_cycles() {
        composeTestRule.onNodeWithTag("Key_ALT").assertIsNotSelected()
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsSelected()
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsNotSelected()
    }

    @Test
    fun modifier_bar_scroll_toggle() {
        // SCROLL toggles the auto-scroll-reset lock; the armed state is
        // exposed via the `selected` semantics on the key button.
        composeTestRule.onNodeWithTag("Key_SCROLL").assertIsNotSelected()
        composeTestRule.onNodeWithTag("Key_SCROLL").performClick()
        composeTestRule.onNodeWithTag("Key_SCROLL").assertIsSelected()
        composeTestRule.onNodeWithTag("Key_SCROLL").performClick()
        composeTestRule.onNodeWithTag("Key_SCROLL").assertIsNotSelected()
    }

    @Test
    fun modifier_bar_esc_triggers_action() {
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ESC").performClick()
    }

    @Test
    fun all_fourteen_keys_clickable() {
        val allTags =
            listOf(
                "Key_ESC",
                "Key_DRAWER",
                "Key_SCROLL",
                "Key_HOME",
                "Key_\u2191",
                "Key_END",
                "Key_PGUP",
                "Key_TAB",
                "Key_CTRL",
                "Key_ALT",
                "Key_\u2190",
                "Key_\u2193",
                "Key_\u2192",
                "Key_PGDN",
            )
        allTags.forEach { tag ->
            composeTestRule.onNodeWithTag(tag).assertIsDisplayed()
            composeTestRule.onNodeWithTag(tag).performClick()
        }
    }

    @Test
    fun rapid_press_does_not_crash() {
        repeat(5) {
            composeTestRule.onNodeWithTag("Key_ESC").performTouchInput {
                down(center)
                up()
            }
        }
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun rapid_press_arrow_keys_does_not_crash() {
        repeat(3) {
            composeTestRule.onNodeWithTag("Key_\u2191").performTouchInput {
                down(Offset(width / 2f, height / 2f))
                up()
            }
            composeTestRule.onNodeWithTag("Key_\u2193").performTouchInput {
                down(Offset(width / 2f, height / 2f))
                up()
            }
        }
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    /** 经 View 派发合成触摸：performTouchInput 无法表达 ACTION_CANCEL，取消语义必须走原始 MotionEvent。 */
    private fun dispatchMotion(downTime: Long, action: Int, position: Offset) {
        val content = composeTestRule.activity.findViewById<View>(android.R.id.content)
        val eventTime = SystemClock.uptimeMillis()
        content.post {
            content.dispatchTouchEvent(
                MotionEvent.obtain(downTime, eventTime, action, position.x, position.y, 0),
            )
        }
        Thread.sleep(80L)
    }

    private fun ctrlCenter(): Offset =
        composeTestRule.onNodeWithTag("Key_CTRL").fetchSemanticsNode().boundsInRoot.center

    @Test
    fun modifier_bar_view_tap_triggers_key() {
        // 对照：View 派发 DOWN+UP 必须触发 —— 保证取消测试非假绿。
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, ctrlCenter())
        dispatchMotion(downTime, MotionEvent.ACTION_UP, ctrlCenter())
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsSelected()
    }

    @Test
    fun modifier_bar_touch_cancel_does_not_trigger_key() {
        // 全面屏手势认领向应用下发 ACTION_CANCEL：不得当成抬手触发按键。
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, ctrlCenter())
        dispatchMotion(downTime, MotionEvent.ACTION_CANCEL, ctrlCenter())
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
    }

    @Test
    fun modifier_bar_touch_cancel_on_drawer_does_not_open_drawer() {
        // 长按副动作分支：取消同样吞掉，不触发抽屉打开。
        composeTestRule.onNodeWithTag("SessionDrawer", useUnmergedTree = true).assertIsNotDisplayed()
        val drawerCenter =
            composeTestRule.onNodeWithTag("Key_DRAWER").fetchSemanticsNode().boundsInRoot.center
        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, drawerCenter)
        dispatchMotion(downTime, MotionEvent.ACTION_CANCEL, drawerCenter)
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SessionDrawer", useUnmergedTree = true).assertIsNotDisplayed()
    }

    @Test
    fun modifier_bar_bottom_position_above_gesture_zone() {
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()

        composeTestRule.waitForIdle()

        val activity = composeTestRule.activity
        val windowMetrics = activity.windowManager.maximumWindowMetrics
        val displayHeight = windowMetrics.bounds.height()
        val navBarHeight =
            windowMetrics
                .windowInsets
                .getInsets(Type.navigationBars())
                .bottom

        val barSemantics = composeTestRule.onNodeWithTag("ModifierBar").fetchSemanticsNode()
        val barBounds = barSemantics.boundsInRoot
        val barBottom = barBounds.bottom.toInt()

        val gestureZoneTop = displayHeight - navBarHeight
        assertTrue(
            "ModifierBar bottom ($barBottom) should be above gesture zone top ($gestureZoneTop), " +
                "displayHeight=$displayHeight, navBarHeight=$navBarHeight, barBounds=$barBounds",
            barBottom <= gestureZoneTop,
        )
    }

    @Test
    fun modifier_bar_bounds_are_valid() {
        composeTestRule.waitForIdle()
        val barSemantics = composeTestRule.onNodeWithTag("ModifierBar").fetchSemanticsNode()
        val bounds = barSemantics.boundsInRoot
        assertTrue(
            "ModifierBar bounds must have positive size: $bounds",
            bounds.width > 0f && bounds.height > 0f,
        )
        assertTrue(
            "ModifierBar bottom should be near screen bottom: $bounds",
            bounds.bottom > 0f,
        )
    }
}
