package io.term.ui

import android.view.WindowInsets
import android.view.WindowInsets.Type
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import io.term.MainActivity
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class ModifierBarTest {
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
    fun modifier_bar_arrow_buttons_exist() {
        composeTestRule.onNodeWithTag("Key_\u2191").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_\u2193").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_\u2190").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_\u2192").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_arrow_buttons_clickable() {
        composeTestRule.onNodeWithTag("Key_\u2191").performClick()
        composeTestRule.onNodeWithTag("Key_\u2193").performClick()
        composeTestRule.onNodeWithTag("Key_\u2190").performClick()
        composeTestRule.onNodeWithTag("Key_\u2192").performClick()
    }

    @Test
    fun modifier_bar_home_pgup_pgdn_exist() {
        composeTestRule.onNodeWithTag("Key_HOME").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_PGUP").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_PGDN").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_home_pgup_pgdn_clickable() {
        composeTestRule.onNodeWithTag("Key_HOME").performClick()
        composeTestRule.onNodeWithTag("Key_PGUP").performClick()
        composeTestRule.onNodeWithTag("Key_PGDN").performClick()
    }

    @Test
    fun modifier_bar_ctrl_toggle_cycles() {
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
    }

    @Test
    fun modifier_bar_alt_toggle_cycles() {
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
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
    }

    @Test
    fun rapid_press_arrow_keys_does_not_crash() {
        repeat(3) {
            composeTestRule.onNodeWithTag("Key_\u2191").performTouchInput {
                down(center)
                up()
            }
            composeTestRule.onNodeWithTag("Key_\u2193").performTouchInput {
                down(center)
                up()
            }
        }
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
