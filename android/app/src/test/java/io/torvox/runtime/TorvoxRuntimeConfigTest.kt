package io.torvox.runtime

import android.content.res.Configuration
import io.mockk.coEvery
import io.mockk.impl.annotations.MockK
import io.mockk.junit4.MockKRule
import io.torvox.settings.SettingsRepository
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
class TorvoxRuntimeConfigTest {
    @get:Rule
    val mockkRule = MockKRule(this)

    @MockK
    private lateinit var settingsRepository: SettingsRepository

    private val testDispatcher = StandardTestDispatcher()
    private lateinit var runtime: TorvoxRuntime

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        runtime = TorvoxRuntime(RuntimeEnvironment.getApplication(), settingsRepository)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun computeFontSizeTenths_returnsCorrectValue() = runTest(testDispatcher) {
        coEvery { settingsRepository.fontSize } returns flowOf(14f)

        val result = runtime.computeFontSizeTenths()

        val density =
            RuntimeEnvironment
                .getApplication()
                .resources.displayMetrics.density
        val expected = ((14f * density) * 10f).toInt().toUInt()
        assertEquals(expected, result)
    }

    @Test
    fun computeFontSizeTenths_smallFontSize() = runTest(testDispatcher) {
        coEvery { settingsRepository.fontSize } returns flowOf(8f)

        val result = runtime.computeFontSizeTenths()

        val density =
            RuntimeEnvironment
                .getApplication()
                .resources.displayMetrics.density
        val expected = ((8f * density) * 10f).toInt().toUInt()
        assertEquals(expected, result)
    }

    @Test
    fun computeFontSizeTenths_largeFontSize() = runTest(testDispatcher) {
        coEvery { settingsRepository.fontSize } returns flowOf(32f)

        val result = runtime.computeFontSizeTenths()

        val density =
            RuntimeEnvironment
                .getApplication()
                .resources.displayMetrics.density
        val expected = ((32f * density) * 10f).toInt().toUInt()
        assertEquals(expected, result)
    }

    @Test
    fun computeFontSizeTenths_densityMultiplier() = runTest(testDispatcher) {
        coEvery { settingsRepository.fontSize } returns flowOf(0f)

        val result = runtime.computeFontSizeTenths()

        assertEquals(0u, result)
    }

    @Test
    fun resolveThemeName_dayTheme_applied() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("day")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Nord")
        coEvery { settingsRepository.appThemeMode } returns flowOf("follow_system")
        setNightMode(Configuration.UI_MODE_NIGHT_YES)

        val result = runtime.resolveThemeName()

        assertEquals("Catppuccin Latte", result)
    }

    @Test
    fun resolveThemeName_nightTheme_applied() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("night")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Nord")
        coEvery { settingsRepository.appThemeMode } returns flowOf("follow_system")
        setNightMode(Configuration.UI_MODE_NIGHT_NO)

        val result = runtime.resolveThemeName()

        assertEquals("Dracula Plus", result)
    }

    @Test
    fun resolveThemeName_followSystem_dark() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("follow_system")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Nord")
        coEvery { settingsRepository.appThemeMode } returns flowOf("follow_system")
        setNightMode(Configuration.UI_MODE_NIGHT_YES)

        val result = runtime.resolveThemeName()

        assertEquals("Dracula Plus", result)
    }

    @Test
    fun resolveThemeName_followSystem_light() = runTest(testDispatcher) {
        coEvery { settingsRepository.themeMode } returns flowOf("follow_system")
        coEvery { settingsRepository.dayThemeName } returns flowOf("Catppuccin Latte")
        coEvery { settingsRepository.nightThemeName } returns flowOf("Dracula Plus")
        coEvery { settingsRepository.themeName } returns flowOf("Nord")
        coEvery { settingsRepository.appThemeMode } returns flowOf("follow_system")
        setNightMode(Configuration.UI_MODE_NIGHT_NO)

        val result = runtime.resolveThemeName()

        assertEquals("Catppuccin Latte", result)
    }

    private fun setNightMode(mode: Int) {
        val context = RuntimeEnvironment.getApplication()
        context.resources.configuration.uiMode =
            (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or mode
    }
}
