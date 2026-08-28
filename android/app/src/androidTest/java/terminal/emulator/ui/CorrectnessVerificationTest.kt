
package terminal.emulator.ui

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.rule.GrantPermissionRule
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runners.MethodSorters
import terminal.emulator.MainActivity
import terminal.emulator.waitForSession

@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class CorrectnessVerificationTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun modifierBar_allKeysExist() {
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
    fun modifierBar_escKeySendsEscape() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_ESC").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
    }

    @Test
    fun drawerButtonExists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("Sessions").assertIsDisplayed()
    }

    @Test
    fun settingsButtonExistsInDrawer() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun settings_showsFontFamily() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        // Three FontFamilySelector rows render (regular/bold/italic) — assert
        // the first one.
        composeTestRule.onAllNodes(hasTestTag("FontFamilySelector")).onFirst().assertIsDisplayed()
    }

    @Test
    fun settings_showsThemeSelector() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        // follow_system mode renders two ThemeSelectors (Day + Night).
        composeTestRule.onAllNodes(hasTestTag("ThemeSelector")).onFirst().assertIsDisplayed()
    }

    @Test
    fun settings_showsFontSizeSlider() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
    }

    @Test
    fun settings_showsBootstrapSection() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onAllNodes(hasTestTag("FontFamilySelector")).onFirst().assertIsDisplayed()
        // "Font Size" label sits above the scroll target; assert before scrolling.
        composeTestRule.onNodeWithText("Font Size").assertIsDisplayed()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onAllNodes(hasTestTag("ThemeSelector")).onFirst().assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
    }

    @Test
    fun terminalScreen_exists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun modifierBar_existsOnTerminal() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun textSearchBar_testTagExists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun themeModeSelector_exists() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("AppThemeSelector"))
        composeTestRule.onNodeWithTag("AppThemeSelector").assertIsDisplayed()
    }

    @Test
    fun terminalThemeFollowSystemSwitch() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeFollowSystemSwitch"))
        // The switch persists in DataStore across tests, so pin it to Off
        // deterministically instead of assuming the default state.
        val switch = composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch")
        val isOn =
            switch.fetchSemanticsNode().config.contains(SemanticsProperties.ToggleableState) &&
                switch.fetchSemanticsNode().config[SemanticsProperties.ToggleableState] == ToggleableState.On
        if (isOn) {
            switch.performClick()
            composeTestRule.waitForIdle()
        }
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").assertIsOff()
    }
}
