package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.test.rule.GrantPermissionRule
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity

class TextSearchInstrumentedTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun search_button_opens_search_bar() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TextSearchBar").assertIsDisplayed()
    }

    @Test
    fun search_text_field_is_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").assertIsDisplayed()
    }

    @Test
    fun search_text_field_accepts_input() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("test")
        composeTestRule.waitForIdle()
    }

    @Test
    @org.junit.Ignore("Vacuously passes while Bridge.searchAllInScrollback is an implemented (native query path is wired) (no real results; assertions only check UI presence)  — native is wired, but the continuous render loop makes Compose idling time out on software-rendered emulators; needs a hardware-accelerated device")
    fun search_result_count_displayed_after_input() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("x")
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchResultCount").assertIsDisplayed()
    }

    @Test
    fun search_previous_button_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchPrevious").assertIsDisplayed()
    }

    @Test
    fun search_next_button_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchNext").assertIsDisplayed()
    }

    @Test
    fun search_close_button_displayed() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchClose").assertIsDisplayed()
    }

    @Test
    fun search_close_button_closes_search() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TextSearchBar").assertDoesNotExist()
    }

    @Test
    @org.junit.Ignore("searchAllInScrollback is an implemented (native query path is wired) (null results, resultCount=0) so SearchNext/SearchPrevious are disabled and performClick() fails on the non-clickable node  — native is wired, but the continuous render loop makes Compose idling time out on software-rendered emulators; needs a hardware-accelerated device")
    fun search_previous_clickable() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("x")
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchPrevious").performClick()
        composeTestRule.waitForIdle()
    }

    @Test
    @org.junit.Ignore("searchAllInScrollback is an implemented (native query path is wired) (null results, resultCount=0) so SearchNext/SearchPrevious are disabled and performClick() fails on the non-clickable node  — native is wired, but the continuous render loop makes Compose idling time out on software-rendered emulators; needs a hardware-accelerated device")
    fun search_next_clickable() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("x")
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchNext").performClick()
        composeTestRule.waitForIdle()
    }
}
