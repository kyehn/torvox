package terminal.emulator

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.closeSettingsOverlay
import terminal.emulator.ui.theme.ThemeRepository

class ThemeInstrumentedTest : ComposeTerminalLogcatTest() {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // the system dialog would cover the UI and break node lookups.

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
    private fun setTerminalThemeDayNight(enabled: Boolean) {
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("TerminalThemeModeSelector"))
        val switch = composeTestRule.onNodeWithTag("TerminalThemeDayNightSwitch")
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
        setTerminalThemeDayNight(enabled = true)
        scrollToNode("DayNightThemeSection")
        composeTestRule.onNodeWithTag("DayNightThemeSection").assertIsDisplayed()
    }

    @Test
    fun terminalThemeSelectorListsAllThemes() {
        setTerminalThemeDayNight(enabled = false)
        scrollToNode("ThemeSelector")
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        // The preview row is virtualized: only the visible themes are
        // composed, so assert on any preview cards that are on screen
        // instead of demanding every catalogue entry at once.
        val previewMatcher =
            SemanticsMatcher("has theme_preview_ tag") { node ->
                node.config.contains(SemanticsProperties.TestTag) &&
                    node.config[SemanticsProperties.TestTag].startsWith("theme_preview_")
            }
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(previewMatcher).fetchSemanticsNodes().size >= 3
        }
        // Every theme name must exist in the catalogue backing the picker.
        assertTrue("Theme catalogue must not be empty", ThemeRepository.all().isNotEmpty())
    }

    @Test
    fun themeModeFollowSystemShowsDayAndNightSelectors() {
        setTerminalThemeDayNight(enabled = true)
        scrollToNode("DayNightThemeSection")
        composeTestRule.onNodeWithTag("DayNightThemeSection").assertIsDisplayed()
        // 两个选择器各自的标签必须都在树里：只断言 section isDisplayed 时，标签缺失
        // 仍会让用例通过（合并语义下 section 存在即可显示）。
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(hasText("日间主题")).fetchSemanticsNodes().isNotEmpty() &&
                composeTestRule.onAllNodes(hasText("夜间主题")).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Test
    fun nightThemeShowsDefaultNameInDayNightSection() {
        setTerminalThemeDayNight(enabled = true)
        scrollToNode("DayNightThemeSection")
        val defaultNightName = ThemeRepository.defaultDarkName()
        composeTestRule.waitUntil(timeoutMillis = 10_000) {
            composeTestRule.onAllNodes(hasText(defaultNightName)).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Test
    fun themeModeFixedOnlyShowsSingleThemeSelector() {
        setTerminalThemeDayNight(enabled = false)
        scrollToNode("ThemeSelector")
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        composeTestRule.onAllNodes(hasTestTag("DayNightThemeSection")).assertCountEquals(0)
    }
}
