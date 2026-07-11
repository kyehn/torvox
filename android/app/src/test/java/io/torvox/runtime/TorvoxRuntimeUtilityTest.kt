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
class TorvoxRuntimeUtilityTest {
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
    fun scrollOffset_defaultsToZero() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        assertEquals(0u, runtime.getScrollOffset())
    }

    @Test
    fun scrollOffset_setOnFreshRuntimeIsSafe() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        runtime.setScrollOffset(10u)
        assertEquals(0u, runtime.getScrollOffset())
    }

    @Test
    fun focusChange_onFreshRuntimeIsSafe() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        runtime.focusChange(true)
        runtime.focusChange(false)
    }

    @Test
    fun currentCwd_onFreshRuntimeReturnsEmpty() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        assertEquals("", runtime.currentCwd())
    }

    @Test
    fun currentSessionIds_onFreshRuntimeReturnsEmpty() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        assertEquals(emptyList<Long>(), runtime.currentSessionIds())
    }

    @Test
    fun currentActiveSessionId_onFreshRuntimeReturnsZero() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        assertEquals(0L, runtime.currentActiveSessionId())
    }

    @Test
    fun setBlitCallback_onFreshRuntimeIsSafe() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        runtime.setBlitCallback { }
    }

    @Test
    fun destroy_afterStopIsSafe() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        runtime.stop()
        runtime.destroy()
    }

    @Test
    fun stateFlow_sessionIdsReflectsCurrentSessions() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }

    @Test
    fun stateFlow_sessionIdsAfterStopIsEmpty() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TorvoxRuntime(context, settingsRepository)
        runtime.stop()
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }
}
