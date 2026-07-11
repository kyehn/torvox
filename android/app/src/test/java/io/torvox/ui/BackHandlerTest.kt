package io.torvox.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.torvox.MainActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BackHandlerTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun drawerCloseOnBackPress() {
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()

        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("SettingsButton").assertIsDisplayed()

        composeTestRule.activity.onBackPressedDispatcher.onBackPressed()

        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("Key_DRAWER").assertIsDisplayed()
    }
}
