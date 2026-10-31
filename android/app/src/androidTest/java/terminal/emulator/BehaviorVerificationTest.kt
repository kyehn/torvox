
package terminal.emulator

import android.view.View
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.closeSettingsOverlay
import terminal.emulator.waitForSession

@RunWith(AndroidJUnit4::class)
class BehaviorVerificationTest : ComposeTerminalLogcatTest() {
    private lateinit var device: UiDevice

    @After
    fun resetScreen() = composeTestRule.closeSettingsOverlay()

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForIdle()
    }

    private fun openSessionDrawer() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        device.waitForIdle(2000)
    }

    private fun openSettings() {
        openSessionDrawer()
        composeTestRule.onNodeWithText("设置").performClick()
        device.waitForIdle(2000)
    }

    private fun scrollToNode(tag: String) {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn", useUnmergedTree = false)
            .performScrollToNode(hasTestTag(tag))
    }

    @Test
    fun terminal_view_displayed_on_launch() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun terminal_view_positive_dimensions() {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val content = activity.findViewById<View>(android.R.id.content)
            assertTrue("Width > 0", content.width > 0)
            assertTrue("Height > 0", content.height > 0)
        }
    }

    @Test
    fun session_drawer_opens_and_shows_session_list() {
        composeTestRule.waitForSession()
        openSessionDrawer()
        composeTestRule.onNodeWithTag("SessionDrawer").assertIsDisplayed()
    }

    @Test
    fun session_drawer_close_on_back_press() {
        composeTestRule.waitForSession()
        openSessionDrawer()
        // The drawer must actually be open, otherwise BACK would finish the
        // activity instead of collapsing the drawer (which surfaces as the
        // misleading "No compose hierarchies" error downstream).
        composeTestRule.onNodeWithTag("SessionDrawer").assertIsDisplayed()
        device.waitForIdle(1000)
        device.pressBack()
        device.waitForIdle(1000)
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @Test
    fun session_drawer_add_session_button_exists() {
        composeTestRule.waitForSession()
        openSessionDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").assertIsDisplayed()
    }

    @Test
    fun settings_screen_sections_displayed() {
        openSettings()

        scrollToNode("FontSizeSlider")
        composeTestRule.onNodeWithTag("FontSizeSlider", useUnmergedTree = false).assertIsDisplayed()
        scrollToNode("AppThemeSelector")
        composeTestRule.onNodeWithTag("AppThemeSelector", useUnmergedTree = false).assertIsDisplayed()
        scrollToNode("TerminalThemeModeSelector")
        composeTestRule
            .onNodeWithTag("TerminalThemeModeSelector", useUnmergedTree = false)
            .assertIsDisplayed()
    }

    @Test
    fun settings_back_navigates_to_terminal() {
        composeTestRule.waitForSession()
        openSessionDrawer()
        composeTestRule.onNodeWithText("设置").performClick()
        device.waitForIdle(2000)

        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()

        composeTestRule.activityRule.scenario.recreate()
        device.waitForIdle(3000)

        composeTestRule.onNodeWithTag("ModifierBar").assertExists()
    }

    @Test
    fun settings_theme_example_displays() {
        openSettings()
        scrollToNode("ThemeSelector")
        // themeMode=follow_system renders two theme selector rows (Day and
        // Night previews) — assert on the first of them.
        composeTestRule.onAllNodes(hasTestTag("ThemeSelector")).onFirst().assertIsDisplayed()
    }

    @Test
    fun app_theme_day_night_buttons_displayed() {
        openSettings()
        scrollToNode("AppThemeSelector")
        composeTestRule.onNodeWithTag("AppTheme_day").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_night").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_follow_system").assertIsDisplayed()
    }

    @Test
    fun app_survives_multiple_pause_resume() {
        composeTestRule.waitForSession()
        for (i in 1..3) {
            composeTestRule.activityRule.scenario.recreate()
            device.waitForIdle(1000)
        }
        device.waitForIdle(2000)
        composeTestRule.onNodeWithTag("TerminalScreen", useUnmergedTree = false).assertIsDisplayed()
    }

    @Test
    fun esc_key_displays_on_modifier_bar() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
    }

    @Test
    fun all_modifier_bar_keys_displayed() {
        composeTestRule.waitForSession()
        // 按逻辑键名定位，不用键帽文字：`extra-keys-style` 默认会把 TAB 画成 ↹。
        val expectedKeys = listOf("ESC", "TAB", "CTRL", "ALT", "HOME", "END", "PGUP", "PGDN")
        for (key in expectedKeys) {
            composeTestRule.onNodeWithTag("Key_$key").assertIsDisplayed()
        }
    }
}
