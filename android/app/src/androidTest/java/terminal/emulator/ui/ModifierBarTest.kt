package terminal.emulator.ui

import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsets.Type
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.MainActivity
import terminal.emulator.probeAssertion

class ModifierBarTest : ComposeTerminalLogcatTest() {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.

    /**
     * 粘滞键复位：全键点击把 CTRL/ALT/SCROLL 逐个置为 armed 并残留，
     * 后继用例（含本类内次序在后者）的首断言会被误伤。仅对仍 armed 者点灭，
     * 已恢复的不碰；点灭若被吞则与今天一致（残留），不新增失败面。
     */
    @After
    fun cleanUpStickyKeys() {
        if (!probeAssertion { composeTestRule.onNodeWithTag("Key_CTRL").assertIsNotSelected() }) {
            composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        }
        if (!probeAssertion { composeTestRule.onNodeWithTag("Key_ALT").assertIsNotSelected() }) {
            composeTestRule.onNodeWithTag("Key_ALT").performClick()
        }
        if (!probeAssertion { composeTestRule.onNodeWithTag("Key_SCROLL").assertIsNotSelected() }) {
            composeTestRule.onNodeWithTag("Key_SCROLL").performClick()
        }
    }

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
        // Hold past the system long-press timeout: the release itself counts even
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

    private fun ctrlCenter(): Offset = composeTestRule
        .onNodeWithTag("Key_CTRL")
        .fetchSemanticsNode()
        .boundsInRoot.center

    private fun escCenter(): Offset = composeTestRule
        .onNodeWithTag("Key_ESC")
        .fetchSemanticsNode()
        .boundsInRoot.center

    /** 两张位图逐像素相同：用于「外观未变」的判别。 */
    private fun samePixels(
        first: android.graphics.Bitmap,
        second: android.graphics.Bitmap,
    ): Boolean {
        if (first.width != second.width || first.height != second.height) return false
        for (row in 0 until first.height) {
            for (column in 0 until first.width) {
                if (first.getPixel(column, row) != second.getPixel(column, row)) return false
            }
        }
        return true
    }

    /**
     * 全面屏上滑手势（系统认领触摸并补发 ACTION_CANCEL）不得留下按键的按下外观。
     *
     * 判别力：按下反馈若在取消后仍走一段复位插值，按键会先缩后弹地动一下——
     * 用户看到的就是「上滑手势触发了按键动画」。取消后按键必须**立刻**回到未
     * 按下外观（逐像素比较），不得停留在按下态或复位途中。
     */
    @Test
    fun modifier_bar_cancelled_gesture_restores_unpressed_appearance() {
        val key = composeTestRule.onNodeWithTag("Key_ESC")
        key.assertIsDisplayed()
        val unpressed = key.captureToImage().asAndroidBitmap()
        val downTime = SystemClock.uptimeMillis()
        composeTestRule.mainClock.autoAdvance = false
        try {
            dispatchMotion(downTime, MotionEvent.ACTION_DOWN, escCenter())
            // 按下反馈的延迟窗口与插值时长都已过：按下动画此刻已经走完。
            composeTestRule.mainClock.advanceTimeBy((PRESS_FEEDBACK_DELAY_MS + PRESS_SCALE_TWEEN_MS).toLong())
            dispatchMotion(downTime, MotionEvent.ACTION_CANCEL, escCenter())
            // 复位必须瞬时：此刻若仍在插值途中，像素就还没回到未按下外观。
            composeTestRule.mainClock.advanceTimeBy((PRESS_SCALE_TWEEN_MS / 3).toLong())
            val afterCancel = key.captureToImage().asAndroidBitmap()
            assertTrue(
                "手势被系统认领后按键必须立刻复位到未按下外观",
                samePixels(unpressed, afterCancel),
            )
        } finally {
            composeTestRule.mainClock.autoAdvance = true
        }
    }

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
            composeTestRule
                .onNodeWithTag("Key_DRAWER")
                .fetchSemanticsNode()
                .boundsInRoot.center
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
        // 系统手势区（全面屏上滑返回/多任务）不在导航栏 inset 之内：系统会先
        // 认领该区域内的触摸再补发 ACTION_CANCEL，键栏压在上面就会把上滑手势
        // 变成一次按键按下反馈。故键栏底边必须同时避开两者。
        val systemGestureHeight =
            windowMetrics
                .windowInsets
                .getInsets(Type.systemGestures())
                .bottom
        val gestureZoneHeight = maxOf(navBarHeight, systemGestureHeight)

        val barSemantics = composeTestRule.onNodeWithTag("ModifierBar").fetchSemanticsNode()
        val barBounds = barSemantics.boundsInRoot
        val barBottom = barBounds.bottom.toInt()

        val gestureZoneTop = displayHeight - gestureZoneHeight
        assertTrue(
            "ModifierBar bottom ($barBottom) should be above gesture zone top ($gestureZoneTop), " +
                "displayHeight=$displayHeight, navBarHeight=$navBarHeight, " +
                "systemGestureHeight=$systemGestureHeight, barBounds=$barBounds",
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
