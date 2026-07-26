package io.term.ui

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.term.RobolectricActivityRule
import io.term.TestActivity
import io.term.ui.theme.BuiltInThemes
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ThemeSettingsComposeTest {
    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    @Test
    fun appThemeSelectorShowsDayNightFollowSystemOptions() {
        var selectedMode = mutableStateOf("day")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                AppThemeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("AppTheme_day").assertExists("Missing day button")
        composeTestRule.onNodeWithTag("AppTheme_night").assertExists("Missing night button")
        composeTestRule.onNodeWithTag("AppTheme_follow_system").assertExists("Missing follow system button")
    }

    @Test
    fun tappingDaySetsDayMode() {
        var selectedMode = mutableStateOf("night")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                AppThemeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("AppTheme_day").performClick()
        assertEquals("day", selectedMode.value)
    }

    @Test
    fun tappingNightSetsNightMode() {
        var selectedMode = mutableStateOf("day")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                AppThemeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("AppTheme_night").performClick()
        assertEquals("night", selectedMode.value)
    }

    @Test
    fun tappingFollowSystemSetsFollowSystem() {
        var selectedMode = mutableStateOf("day")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                AppThemeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("AppTheme_follow_system").performClick()
        assertEquals("follow_system", selectedMode.value)
    }

    @Test
    fun terminalThemeToggleShowsFollowSystemSwitch() {
        var selectedMode = mutableStateOf("follow_system")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                TerminalThemeModeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("TerminalThemeModeSelector").assertExists("Missing theme mode selector")
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").assertExists("Missing follow system switch")
    }

    @Test
    fun togglingSwitchToOffSetsFixed() {
        var selectedMode = mutableStateOf("follow_system")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                TerminalThemeModeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").performClick()
        assertEquals("fixed", selectedMode.value)
    }

    @Test
    fun togglingSwitchToOnSetsFollowSystem() {
        var selectedMode = mutableStateOf("fixed")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                TerminalThemeModeSelector(
                    selectedMode = selectedMode.value,
                    onModeSelected = { selectedMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }
        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").performClick()
        assertEquals("follow_system", selectedMode.value)
    }

    @Test
    fun themeSelectorScrollsHorizontally() {
        var selectedTheme = mutableStateOf("Catppuccin Mocha")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                ThemeSelector(
                    label = "Test Theme",
                    themes = BuiltInThemes.all.take(3),
                    selectedTheme = selectedTheme.value,
                    onThemeSelected = { selectedTheme.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    secondaryText = androidx.compose.ui.graphics.Color.Gray,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                )
            }
        }
        composeTestRule.onNodeWithTag("ThemeSelector").assertExists()
        composeTestRule.onNodeWithTag("theme_preview_Catppuccin Mocha").assertExists()
    }

    @Test
    fun tappingThemePreviewSelectsThatTheme() {
        var selectedTheme = mutableStateOf("Nord")
        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                ThemeSelector(
                    label = "Test Theme",
                    themes = BuiltInThemes.darkThemes.take(3),
                    selectedTheme = selectedTheme.value,
                    onThemeSelected = { selectedTheme.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    secondaryText = androidx.compose.ui.graphics.Color.Gray,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                )
            }
        }
        composeTestRule.onNodeWithTag("theme_preview_Nord").performClick()
        assertEquals("Nord", selectedTheme.value)
    }
}
