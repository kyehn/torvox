
package io.term.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import io.term.MainActivity
import io.term.waitForSession
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runners.MethodSorters

@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class CorrectnessVerificationTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun test01_modifierBar_allKeysExist() {
        composeTestRule.waitForSession()
        val expectedKeys =
            listOf(
                "Key_ESC",
                "Key_DRAWER",
                "Key_SCROLL",
                "Key_HOME",
                "Key_↑",
                "Key_END",
                "Key_PGUP",
                "Key_TAB",
                "Key_CTRL",
                "Key_ALT",
                "Key_←",
                "Key_↓",
                "Key_→",
                "Key_PGDN",
            )
        for (key in expectedKeys) {
            composeTestRule.onNodeWithTag(key).assertIsDisplayed()
        }
    }

    @Test
    fun test02_modifierBar_escKeySendsEscape() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_ESC").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
    }

    @Test
    fun test03_drawerButtonExists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Sessions").assertIsDisplayed()
    }

    @Test
    fun test04_settingsButtonExistsInDrawer() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun test05_settings_showsFontFamily() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("FontFamilySelector").assertIsDisplayed()
    }

    @Test
    fun test06_settings_showsThemeSelector() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
    }

    @Test
    fun test07_settings_showsFontSizeSlider() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
    }

    @Test
    fun test08_settings_showsBootstrapSection() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("FontFamilySelector").assertIsDisplayed()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        composeTestRule.onNodeWithText("Font Size").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun test09_terminalScreen_exists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun test10_modifierBar_existsOnTerminal() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun test11_textSearchBar_testTagExists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun test12_themeModeSelector_exists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("AppThemeSelector").assertIsDisplayed()
    }

    @Test
    fun test13_terminalThemeFollowSystemSwitch() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeFollowSystemSwitch"))
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").assertIsOff()
    }
}
