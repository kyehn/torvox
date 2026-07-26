package io.term.runtime

import android.content.res.Configuration
import io.mockk.coEvery
import io.mockk.impl.annotations.MockK
import io.mockk.junit4.MockKRule
import io.term.settings.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class TorvoxRuntimeTest {
    @get:Rule
    val mockkRule = MockKRule(this)

    @MockK
    private lateinit var settingsRepository: SettingsRepository

    private val testDispatcher = StandardTestDispatcher()
    private lateinit var runtime: TerminalRuntime

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        runtime = TerminalRuntime(RuntimeEnvironment.getApplication(), settingsRepository)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
        setNightMode(Configuration.UI_MODE_NIGHT_NO)
    }

    @Test
    fun resolveThemeName_appThemeMode_day() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("follow_system")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.appThemeMode } returns flowOf("day")
        setNightMode(Configuration.UI_MODE_NIGHT_YES)

        val result = runtime.resolveThemeName()

        assertEquals("Catppuccin Latte", result)
    }

    @Test
    fun resolveThemeName_appThemeMode_night() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("follow_system")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.appThemeMode } returns flowOf("night")
        setNightMode(Configuration.UI_MODE_NIGHT_NO)

        val result = runtime.resolveThemeName()

        assertEquals("Dracula Plus", result)
    }

    @Test
    fun resolveThemeName_appThemeMode_follow_system() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("follow_system")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.appThemeMode } returns flowOf("follow_system")
        setNightMode(Configuration.UI_MODE_NIGHT_YES)

        val result = runtime.resolveThemeName()

        assertEquals("Dracula Plus", result)
    }

    @Test
    fun resolveThemeName_fixed_not_affected() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("fixed")
        coEvery { settingsRepository.themeName } returns flowOf("Nord")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.appThemeMode } returns flowOf("day")
        setNightMode(Configuration.UI_MODE_NIGHT_YES)

        val result = runtime.resolveThemeName()

        assertEquals("Nord", result)
    }

    private fun setNightMode(mode: Int) {
        val context = RuntimeEnvironment.getApplication()
        context.resources.configuration.uiMode =
            (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or mode
    }
}
