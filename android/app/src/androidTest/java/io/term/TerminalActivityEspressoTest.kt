package io.term

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.isRoot
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class TerminalActivityEspressoTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun appLaunchesAndShowsTerminalScreen() {
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun terminalContentIsDisplayed() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun modifierBarKeysAreVisible() {
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun drawerCanBeOpened() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.onNodeWithText("New session").assertIsDisplayed()
    }

    @Test
    fun settingsCanBeNavigatedTo() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.onNodeWithText("Settings").performClick()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun activityContentViewIsDisplayed() {
        onView(isRoot()).check(matches(isDisplayed()))
    }
}
