package io.term.runtime

import io.term.settings.SettingsDataStoreProvider
import io.term.settings.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
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
class SelectionAccentColorTest {
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
    fun accentColor_defaultsToBlue() {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        assertEquals(0xFF2196F3.toInt(), runtime.accentColor)
    }

    @Test
    fun accentColor_canBeUpdated() {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.accentColor = 0xFFFF0000.toInt()
        assertEquals(0xFFFF0000.toInt(), runtime.accentColor)
    }

    @Test
    fun accentColor_tracksThemeAnsi5() {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.accentColor = 0xFFD78700.toInt()
        assertEquals(0xFFD78700.toInt(), runtime.accentColor)
    }
}
