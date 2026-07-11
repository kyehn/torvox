package io.torvox.ui

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import com.github.takahirom.roborazzi.RoborazziRule
import io.mockk.coEvery
import io.mockk.mockk
import io.torvox.RobolectricActivityRule
import io.torvox.TestActivity
import io.torvox.runtime.TorvoxRuntime
import io.torvox.settings.SettingsRepository
import io.torvox.ui.theme.BuiltInThemes
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ThemeModeSwitchComposeTest {
    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    @get:Rule
    val roborazziRule =
        RoborazziRule(
            RoborazziRule.Options(
                outputDirectoryPath = "src/test/resources/roborazzi",
            ),
        )

    @Test
    fun appThemeSelectorChangesUpdateTerminalThemeMode() {
        var appThemeMode = mutableStateOf("follow_system")
        var themeMode = mutableStateOf("follow_system")
        var selectedTheme = mutableStateOf("Dracula Plus")

        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                AppThemeSelector(
                    selectedMode = appThemeMode.value,
                    onModeSelected = { appThemeMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }

        composeTestRule.onNodeWithTag("AppTheme_night").performClick()
        assertEquals("night", appThemeMode.value)

        composeTestRule.onNodeWithTag("AppTheme_day").performClick()
        assertEquals("day", appThemeMode.value)

        composeTestRule.onNodeWithTag("AppTheme_follow_system").performClick()
        assertEquals("follow_system", appThemeMode.value)
    }

    @Test
    fun terminalThemeFollowSystemUsesCorrectDayNightTheme() {
        val dayTheme = BuiltInThemes.byName("Catppuccin Latte")
        val nightTheme = BuiltInThemes.byName("Dracula Plus")
        assertEquals("Catppuccin Latte", dayTheme.name)
        assertEquals("Dracula Plus", nightTheme.name)
        assert(dayTheme.background != nightTheme.background) {
            "Day and night themes must have different backgrounds"
        }
    }

    @Test
    fun themeModeFixedUsesSingleThemeOnly() {
        var themeMode = mutableStateOf("fixed")

        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                TerminalThemeModeSelector(
                    selectedMode = themeMode.value,
                    onModeSelected = { themeMode.value = it },
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                    accentColor = androidx.compose.ui.graphics.Color.Blue,
                )
            }
        }

        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").performClick()
        assertEquals("follow_system", themeMode.value)

        composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch").performClick()
        assertEquals("fixed", themeMode.value)
    }

    @Test
    fun themeSelectorRendersBuiltInThemes() {
        val allThemes = BuiltInThemes.all
        assertTrue(allThemes.size >= 16)

        composeTestRule.setContent {
            androidx.compose.material3.MaterialTheme {
                ThemeSelector(
                    label = "Theme",
                    themes = allThemes,
                    selectedTheme = "Dracula Plus",
                    onThemeSelected = {},
                    textColor = androidx.compose.ui.graphics.Color.Black,
                    secondaryText = androidx.compose.ui.graphics.Color.Gray,
                    cardBackground = androidx.compose.ui.graphics.Color.LightGray,
                )
            }
        }

        composeTestRule.onNodeWithTag("ThemeSelector").assertExists()
        composeTestRule.onNodeWithTag("theme_preview_Dracula Plus").assertExists()
        composeTestRule.onNodeWithTag("theme_preview_Catppuccin Mocha").assertExists()
    }

    @Test
    fun terminalThemeResolvedCorrectlyForEachMode() = runTest {
        val mockRepo = mockk<SettingsRepository>()
        coEvery { mockRepo.themeMode } returns flowOf("follow_system")
        coEvery { mockRepo.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { mockRepo.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { mockRepo.themeName } returns flowOf("Nord")
        coEvery { mockRepo.appThemeMode } returns flowOf("follow_system")

        val runtime = TorvoxRuntime(RuntimeEnvironment.getApplication(), mockRepo)

        assertEquals("Catppuccin Latte", runtime.resolveThemeName())

        coEvery { mockRepo.appThemeMode } returns flowOf("day")
        assertEquals("Catppuccin Latte", runtime.resolveThemeName())

        coEvery { mockRepo.appThemeMode } returns flowOf("night")
        assertEquals("Dracula Plus", runtime.resolveThemeName())

        coEvery { mockRepo.appThemeMode } returns flowOf("follow_system")
        coEvery { mockRepo.themeMode } returns flowOf("fixed")
        assertEquals("Nord", runtime.resolveThemeName())
    }
}
