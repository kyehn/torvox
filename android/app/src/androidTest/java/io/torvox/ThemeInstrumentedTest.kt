package io.torvox

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import io.torvox.ui.theme.BuiltInThemes
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class ThemeInstrumentedTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("AppThemeSelector"))
        composeTestRule.onNodeWithTag("AppTheme_follow_system").performClick()
        composeTestRule.waitForIdle()
    }

    @Test
    fun settingsShowsAppThemeSelector() {
        composeTestRule.onNodeWithTag("AppThemeSelector").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_day").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_night").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_follow_system").assertIsDisplayed()
    }

    @Test
    fun settingsShowsDayNightThemeSelectors() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("DayNightThemeSection"))
        composeTestRule.onNodeWithTag("DayNightThemeSection").assertIsDisplayed()
    }

    @Test
    fun terminalThemeSelectorListsAllThemes() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()

        BuiltInThemes.all.forEach { theme ->
            composeTestRule.onNodeWithText(theme.name).assertExists()
        }
    }

    @Test
    fun switchingAppThemeToDayShowsDefaultDayThemeName() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("AppThemeSelector"))
        composeTestRule.onNodeWithTag("AppTheme_day").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("DayNightThemeSection"))
        composeTestRule.onNodeWithText(BuiltInThemes.byName("Catppuccin Latte").name).assertExists()
    }

    @Test
    fun switchingAppThemeToNightShowsDefaultNightThemeName() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("AppThemeSelector"))
        composeTestRule.onNodeWithTag("AppTheme_night").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("DayNightThemeSection"))
        composeTestRule.onNodeWithText(BuiltInThemes.byName("Dracula Plus").name).assertExists()
    }

    @Test
    fun themeModeFixedOnlyShowsSingleThemeSelector() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeModeSelector"))
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
    }

    @Test
    fun themeModeFollowSystemShowsDayAndNightSelectors() {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeModeSelector"))
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").performClick()
        composeTestRule.waitForIdle()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("DayNightThemeSection"))
        composeTestRule.onNodeWithTag("DayNightThemeSection").assertIsDisplayed()
    }
}
