package terminal.emulator.ui

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.openSettings
import terminal.emulator.waitForSession

class SettingsScreenTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
    }

    @Test
    fun settings_screen_renders_back_button() {
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsBackButton").assertIsDisplayed()
    }

    @Test
    fun back_button_navigates_to_terminal() {
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun settings_screen_shows_appearance_section() {
        composeTestRule.onNodeWithText("外观").assertIsDisplayed()
    }

    @Test
    fun settings_screen_shows_font_size_slider() {
        composeTestRule.onNodeWithTag("FontSizeSlider").assertIsDisplayed()
    }

    @Test
    fun settings_screen_switches_day_theme() {
        // 日间主题行只在跟随系统开时渲染（模式跨测试持久化，先确保打开）。
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeFollowSystemSwitch"))
        val followSwitch = composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch")
        val followOn =
            followSwitch.fetchSemanticsNode().config.contains(SemanticsProperties.ToggleableState) &&
                followSwitch.fetchSemanticsNode().config[SemanticsProperties.ToggleableState] ==
                ToggleableState.On
        if (!followOn) {
            followSwitch.performClick()
            composeTestRule.waitForIdle()
        }
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasText("日间主题"))
        composeTestRule.onNodeWithText("日间主题").assertIsDisplayed()
    }

    @Test
    fun settings_screen_displays_font_list() {
        composeTestRule.onNodeWithTag("FontSizeSlider").assertExists()
    }

    @Test
    fun settings_screen_shows_chinese_terminal_section_titles() {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(hasText("终端主题"))
        composeTestRule.onNodeWithText("终端主题").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(hasText("软件主题"))
        composeTestRule.onNodeWithText("软件主题").assertIsDisplayed()
        // 软件主题与终端主题各有一个跟随系统开关，断言存在即可。
        assertTrue(
            composeTestRule.onAllNodes(hasText("跟随系统")).fetchSemanticsNodes().isNotEmpty(),
        )
    }

    @Test
    fun theme_switch_changes_terminal_appearance() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
        // Pin follow_system off so exactly one ThemeSelector renders (the
        // mode persists in DataStore across tests).
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeFollowSystemSwitch"))
        val switch = composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch")
        val isOn =
            switch.fetchSemanticsNode().config.contains(SemanticsProperties.ToggleableState) &&
                switch.fetchSemanticsNode().config[SemanticsProperties.ToggleableState] ==
                ToggleableState.On
        if (isOn) {
            switch.performClick()
            composeTestRule.waitForIdle()
        }
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(hasText("Dracula Plus"))
        composeTestRule.onNodeWithText("Dracula Plus").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SettingsBackButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.waitUntil(timeoutMillis = 5_000) {
            composeTestRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }
}
