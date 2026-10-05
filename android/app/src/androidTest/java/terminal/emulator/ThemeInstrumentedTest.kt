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
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.closeSettingsOverlay
import terminal.emulator.ui.theme.BuiltInThemes

class ThemeInstrumentedTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private var themeState: terminal.emulator.settings.SettingsRepository.SettingsState? = null

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            themeState = activity.terminalViewModel.settings.value
        }
        composeTestRule.openSettings()
    }

    /**
     * 还原主题设置。
     *
     * 主题名与跟随开关都持久化在 DataStore，且 `:app:connectedDebugAndroidTest`
     * 全部用例同进程。不还原即把**后续所有**用例留在本类选中的主题下：浅色主题
     * 里块光标是深色的，按绝对亮度判「光标格变亮」的像素用例会整片判红
     * （实测亮度 0），而判红原因与光标渲染无关。
     */
    @After
    fun tearDown() {
        composeTestRule.closeSettingsOverlay()
        val before = themeState ?: return
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val viewModel = activity.terminalViewModel
            viewModel.setThemeMode(before.themeMode)
            viewModel.setThemeName(before.themeName)
            viewModel.setDayThemeName(before.dayThemeName)
            viewModel.setNightThemeName(before.nightThemeName)
        }
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
            composeTestRule.onAllNodes(hasText("日间主题")).fetchSemanticsNodes().isNotEmpty()
        }
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(hasText("夜间主题")).fetchSemanticsNodes().isNotEmpty()
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
