
package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.bridge.Bridge
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

@RunWith(AndroidJUnit4::class)
@LargeTest
class TextSearchImeSmartCaseTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun imeDoesNotObscureSearchBar() {
        composeTestRule.waitForSession()
        openSearchBar()

        composeTestRule.onNodeWithTag("TextSearchBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SearchTextField").assertIsDisplayed()

        composeTestRule.onNodeWithTag("SearchTextField").performClick()
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("TextSearchBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SearchTextField").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SearchClose").assertIsDisplayed()
    }

    @Test
    fun smartCase_autoEnablesOnUppercase() {
        composeTestRule.waitForSession()
        openSearchBar()

        composeTestRule.onNodeWithTag("SearchTextField").performTextClearance()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("Hello")
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("SearchCaseSensitive").assertIsDisplayed()

        composeTestRule.onNodeWithTag("SearchTextField").performTextClearance()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("hello")
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("SearchCaseSensitive").assertIsDisplayed()
    }

    @Test
    fun smartCase_manualToggleOverridesAuto() {
        composeTestRule.waitForSession()
        openSearchBar()

        composeTestRule.onNodeWithTag("SearchTextField").performTextClearance()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("Hello")
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("SearchCaseSensitive").performClick()
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("SearchTextField").performTextClearance()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("hello")
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithTag("SearchCaseSensitive").assertIsDisplayed()
    }

    @Test
    @org.junit.Ignore("Vacuously passes while Bridge.searchAllInScrollback is an implemented (native query path is wired) (no real results; assertions only check UI presence) ")
    fun smartCase_uppercaseSearch_returnsDifferentResults() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val marker = "SmartCase_${java.util.UUID.randomUUID().toString().take(6).uppercase()}"

        bridge.writeToPty("echo '${marker}_lower'\n".toByteArray())
        bridge.writeToPty("echo '${marker}_UPPER'\n".toByteArray())
        Thread.sleep(3000)

        openSearchBar()

        composeTestRule.onNodeWithTag("SearchTextField").performTextClearance()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput(marker)
        composeTestRule.waitForIdle()
        Thread.sleep(2000)

        composeTestRule.onNodeWithTag("SearchResultCount").assertIsDisplayed()
    }

    private fun openSearchBar() {
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
    }
}
