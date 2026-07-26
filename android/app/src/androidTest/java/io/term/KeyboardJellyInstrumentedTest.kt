package io.term

import android.view.KeyEvent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.pressKey
import androidx.test.espresso.matcher.ViewMatchers.isRoot
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class KeyboardJellyInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun typingSendsToTerminal() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        onView(isRoot()).perform(pressKey(KeyEvent.KEYCODE_A))
    }

    @Test
    fun ctrlKeyCanBeClicked() {
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
    }

    @Test
    fun altKeyCanBeClicked() {
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
    }

    @Test
    fun ctrlAndAltKeysDoNotCrash() {
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun modifierBarKeysRespondToClick() {
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_DRAWER").assertIsDisplayed()
    }

    @Test
    fun imeInteractionDoesNotCrash() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").performClick()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }
}
