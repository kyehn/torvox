package io.torvox.runtime

import io.torvox.settings.SettingsDataStoreProvider
import io.torvox.settings.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class TorvoxRuntimeFontSizeTest {
    private val testDispatcher = StandardTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun fontSizeTenthsEqualsSliderTimesDensityTimesTen() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        settingsRepository.setFontSize(18f)
        val runtime = TorvoxRuntime(context, settingsRepository)
        val result = runtime.computeFontSizeTenths()
        assertEquals(180u, result)
    }

    @Test
    fun fontSizeIsLinearAcrossSliderRange() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)

        settingsRepository.setFontSize(10f)
        val result10 = runtime.computeFontSizeTenths()

        settingsRepository.setFontSize(20f)
        val result20 = runtime.computeFontSizeTenths()

        assertEquals(2.0f, result20.toFloat() / result10.toFloat(), 0.01f)
    }

    @Test
    fun fontSizeTenthsForMinimumSlider() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        settingsRepository.setFontSize(8f)
        val runtime = TorvoxRuntime(context, settingsRepository)
        val result = runtime.computeFontSizeTenths()
        assertEquals(80u, result)
    }

    @Test
    fun fontSizeTenthsForMaximumSlider() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        settingsRepository.setFontSize(48f)
        val runtime = TorvoxRuntime(context, settingsRepository)
        val result = runtime.computeFontSizeTenths()
        assertEquals(480u, result)
    }
}
