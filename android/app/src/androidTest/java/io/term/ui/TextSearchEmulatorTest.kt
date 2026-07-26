
package io.term.ui

import android.os.SystemClock
import android.view.KeyEvent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.term.MainActivity
import io.term.openDrawer
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class TextSearchEmulatorTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun testSearchButtonInDrawer_opensSearchBar() {
        composeTestRule.waitForIdle()
        openSearchBar()
        composeTestRule.onNodeWithTag("SearchTextField").assertIsDisplayed()
    }

    @Test
    fun testCtrlF_doesNotOpenSearchBar() {
        composeTestRule.waitForIdle()
        sendCtrlF()
        composeTestRule.waitForIdle()

        assertEquals(
            "Ctrl+F should not open search bar when shortcut is disabled",
            0,
            composeTestRule.onAllNodesWithTag("TextSearchBar").fetchSemanticsNodes().size,
        )
    }

    @Test
    fun testSearchCaseSensitivity() {
        composeTestRule.waitForIdle()
        openSearchBar()
        composeTestRule.onNodeWithTag("SearchCaseSensitive").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TextSearchBar").assertIsDisplayed()
    }

    @Test
    fun testSearchNavigation() {
        composeTestRule.waitForIdle()
        openSearchBar()
        composeTestRule.onNodeWithTag("SearchTextField").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SearchClose").performClick()
    }

    @Test
    fun testSearchInput_acceptsTyping() {
        composeTestRule.waitForIdle()
        openSearchBar()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("test")
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextClearance()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("search")
        composeTestRule.waitForIdle()
    }

    private fun openSearchBar() {
        composeTestRule.waitForIdle()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
    }

    private fun sendCtrlF() {
        composeTestRule.runOnUiThread {
            val t = SystemClock.uptimeMillis()
            composeTestRule.activity.dispatchKeyEvent(
                KeyEvent(t, t, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_F, 0, KeyEvent.META_CTRL_ON),
            )
            composeTestRule.activity.dispatchKeyEvent(
                KeyEvent(t, t + 10, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_F, 0, KeyEvent.META_CTRL_ON),
            )
        }
    }
}
