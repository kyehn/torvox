package terminal.emulator

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.ui.theme.BuiltInThemes

class ThemeInstrumentedTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.openSettings()
    }

    /**
     * Pins the terminal theme mode (the switch feeding `theme_mode`, which
     * decides whether the Day/Night section or a single theme selector is
     * shown). The mode persists in DataStore across tests, so every test
     * states its precondition explicitly instead of relying on run order.
     */
    private fun setTerminalThemeFollowSystem(enabled: Boolean) {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeModeSelector"))
        val switch = composeTestRule.onNodeWithTag("TerminalThemeFollowSystemSwitch")
        val isOn =
            switch.fetchSemanticsNode().config.contains(SemanticsProperties.ToggleableState) &&
                switch.fetchSemanticsNode().config[SemanticsProperties.ToggleableState] == ToggleableState.On
        if (isOn != enabled) {
            switch.performClick()
        }
        composeTestRule.waitForIdle()
        if (enabled) {
            composeTestRule.waitUntil(timeoutMillis = 10_000) {
                composeTestRule.onAllNodes(hasTestTag("DayNightThemeSection")).fetchSemanticsNodes().isNotEmpty()
            }
        }
    }

    private fun scrollToNode(tag: String) {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(hasTestTag(tag))
    }

    @Test
    fun settingsShowsAppThemeSelector() {
        scrollToNode("AppThemeSelector")
        composeTestRule.onNodeWithTag("AppThemeSelector").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_day").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_night").assertIsDisplayed()
        composeTestRule.onNodeWithTag("AppTheme_follow_system").assertIsDisplayed()
    }

    @Test
    fun settingsShowsDayNightThemeSelectors() {
        setTerminalThemeFollowSystem(enabled = true)
        scrollToNode("DayNightThemeSection")
        composeTestRule.onNodeWithTag("DayNightThemeSection").assertIsDisplayed()
    }

    @Test
    fun terminalThemeSelectorListsAllThemes() {
        setTerminalThemeFollowSystem(enabled = false)
        scrollToNode("ThemeSelector")
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        // The preview row is virtualized: only the visible themes are
        // composed, so assert on any preview cards that are on screen
        // instead of demanding every BuiltInThemes entry at once.
        val previewMatcher =
            SemanticsMatcher("has theme_preview_ tag") { node ->
                node.config.contains(SemanticsProperties.TestTag) &&
                    node.config[SemanticsProperties.TestTag].startsWith("theme_preview_")
            }
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(previewMatcher).fetchSemanticsNodes().size >= 3
        }
        // Every theme name must exist in the catalogue backing the picker.
        assertTrue("BuiltInThemes catalogue must not be empty", BuiltInThemes.all.isNotEmpty())
    }

    @Test
    fun dayThemeShowsDefaultNameInDayNightSection() {
        setTerminalThemeFollowSystem(enabled = true)
        scrollToNode("DayNightThemeSection")
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(hasText("Day Theme")).fetchSemanticsNodes().isNotEmpty()
        }
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(hasText("Night Theme")).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Test
    fun nightThemeShowsDefaultNameInDayNightSection() {
        setTerminalThemeFollowSystem(enabled = true)
        scrollToNode("DayNightThemeSection")
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(hasText("Dracula Plus")).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Test
    fun themeModeFixedOnlyShowsSingleThemeSelector() {
        setTerminalThemeFollowSystem(enabled = false)
        scrollToNode("ThemeSelector")
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        composeTestRule.onAllNodes(hasTestTag("DayNightThemeSection")).assertCountEquals(0)
    }

    @Test
    fun themeModeFollowSystemShowsDayAndNightSelectors() {
        setTerminalThemeFollowSystem(enabled = true)
        scrollToNode("DayNightThemeSection")
        composeTestRule.onNodeWithTag("DayNightThemeSection").assertIsDisplayed()
    }
}
