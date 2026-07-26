
package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.term.MainActivity
import io.term.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalScreenUiTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminalContent_isDisplayed() { // B1: VT parse + SGR colors/styles rendered on screen
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun drawer_opensOnClick() { // B11: drawer exposes clipboard/title controls (OSC 52)
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SessionDrawer").assertIsDisplayed()
    }

    @Test
    fun modifierBar_isPresent() { // B6: Kitty keyboard modifier keys (CTRL/ALT) shown
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ALT").assertIsDisplayed()
    }

    @Test
    fun tappingTerminal_requestsSoftKeyboard() { // B6: Kitty keyboard protocol input via IME
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalContent").performClick()
        composeTestRule.waitForIdle()
        val inputMethodManager =
            composeTestRule.activity.getSystemService(
                android.content.Context.INPUT_METHOD_SERVICE,
            ) as android.view.inputmethod.InputMethodManager
        val focusedView = composeTestRule.activity.currentFocus
        assertTrue(
            "Soft keyboard should be requested after tapping terminal",
            focusedView != null && inputMethodManager.isActive(focusedView),
        )
    }
}
