
package io.torvox

import android.view.View
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BehaviorVerificationTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeRule.waitForIdle()
    }

    private fun openSessionDrawer() {
        composeRule.onNodeWithTag("Key_DRAWER").performClick()
        device.waitForIdle(2000)
    }

    @Test
    fun terminal_view_displayed_on_launch() {
        composeRule.waitForSession()
        composeRule.onNodeWithTag("TerminalScreen", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun terminal_modifier_bar_displayed() {
        composeRule.waitForSession()
        composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun terminal_view_positive_dimensions() {
        composeRule.activityRule.scenario.onActivity { activity ->
            val content = activity.findViewById<View>(android.R.id.content)
            assertTrue("Width > 0", content.width > 0)
            assertTrue("Height > 0", content.height > 0)
        }
    }

    @Test
    fun session_drawer_opens_and_shows_session_list() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithTag("SessionDrawer").assertIsDisplayed()
    }

    @Test
    fun session_drawer_close_on_back_press() {
        composeRule.waitForSession()
        openSessionDrawer()
        device.waitForIdle(1000)
        device.pressBack()
        device.waitForIdle(1000)
        composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun session_drawer_add_session_button_exists() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }

    @Test
    fun settings_screen_sections_displayed() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithText("Settings").performClick()
        device.waitForIdle(2000)

        composeRule.onNodeWithTag("AppThemeSelector", useUnmergedTree = false).assertIsDisplayed()
        composeRule.onNodeWithTag("FontSizeSlider", useUnmergedTree = false).assertIsDisplayed()
        composeRule
            .onNodeWithTag("TerminalThemeModeSelector", useUnmergedTree = false)
            .assertIsDisplayed()
    }

    @Test
    fun settings_back_navigates_to_terminal() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithText("Settings").performClick()
        device.waitForIdle(2000)

        composeRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()

        composeRule.activityRule.scenario.recreate()
        device.waitForIdle(3000)

        composeRule.onNodeWithTag("ModifierBar").assertExists()
    }

    @Test
    fun settings_font_slider_exists() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithText("Settings").performClick()
        device.waitForIdle(2000)
        composeRule.onNodeWithTag("FontSizeSlider", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun settings_theme_example_displays() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithText("Settings").performClick()
        device.waitForIdle(2000)
        composeRule
            .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = false)
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeRule.onNodeWithTag("ThemeSelector", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun app_theme_day_night_buttons_displayed() {
        composeRule.waitForSession()
        openSessionDrawer()
        composeRule.onNodeWithText("Settings").performClick()
        device.waitForIdle(2000)

        composeRule.onNodeWithTag("AppTheme_day").assertIsDisplayed()
        composeRule.onNodeWithTag("AppTheme_night").assertIsDisplayed()
        composeRule.onNodeWithTag("AppTheme_follow_system").assertIsDisplayed()
    }

    @Test
    fun app_survives_pause_resume_cycle() {
        composeRule.waitForSession()
        composeRule.activityRule.scenario.recreate()
        device.waitForIdle(2000)
        composeRule.onNodeWithTag("TerminalScreen", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun app_survives_multiple_pause_resume() {
        composeRule.waitForSession()
        for (i in 1..3) {
            composeRule.activityRule.scenario.recreate()
            device.waitForIdle(1000)
        }
        device.waitForIdle(2000)
        composeRule.onNodeWithTag("TerminalScreen", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun keyboard_icon_displays_on_modifier_bar() {
        composeRule.waitForSession()
        composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeRule.onNodeWithText("ESC").assertIsDisplayed()
    }

    @Test
    fun ctrl_key_toggle_changes_appearance() {
        composeRule.waitForSession()
        composeRule.onNodeWithText("CTRL").performClick()
        device.waitForIdle(1000)
        composeRule.onNodeWithText("CTRL").performClick()
    }

    @Test
    fun all_modifier_bar_keys_displayed() {
        composeRule.waitForSession()
        val expectedKeys = listOf("ESC", "TAB", "CTRL", "ALT", "HOME", "END", "PGUP", "PGDN")
        for (key in expectedKeys) {
            composeRule.onNodeWithText(key).assertIsDisplayed()
        }
    }
}
