package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import io.torvox.MainActivity
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
    fun modifier_bar_esc_key_exists() {
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_ctrl_key_exists() {
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
    }

    @Test
    fun modifier_bar_alt_key_exists() {
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
    }
}
