package io.term.runtime

import io.term.settings.SettingsDataStoreProvider
import io.term.settings.SettingsRepository
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
class TorvoxRuntimeLifecycleTest {
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
    fun stop_on_fresh_runtime_does_not_crash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.stop()
        assertEquals(false, runtime.state.value.isRunning)
    }

    @Test
    fun writeToPty_after_stop_does_not_crash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.stop()
        runtime.writeToPty(byteArrayOf(0x48, 0x69))
        assertEquals(false, runtime.state.value.isRunning)
    }

    @Test
    fun double_stop_is_idempotent() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.stop()
        runtime.stop()
        assertEquals(false, runtime.state.value.isRunning)
    }

    @Test
    fun runtimeStartsWithExpectedDefaults() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        val state = runtime.state.value
        assertEquals(false, state.isRunning)
        assertEquals(0L, state.activeSessionId)
        assertEquals(emptyList<Long>(), state.sessionIds)
    }

    @Test
    fun createSession_returnsMinusOneOnZeroWidth() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        val surface = android.view.Surface(android.graphics.SurfaceTexture(0))
        val newId = runtime.createSession(surface, 0, 720)
        assertEquals(-1L, newId)
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }

    @Test
    fun createSession_returnsMinusOneOnZeroHeight() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        val surface = android.view.Surface(android.graphics.SurfaceTexture(0))
        val newId = runtime.createSession(surface, 1080, 0)
        assertEquals(-1L, newId)
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }

    @Test
    fun createSession_returnsMinusOneOnDestroyedSurface() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        val surface = android.view.Surface(android.graphics.SurfaceTexture(0))
        surface.release()
        val newId = runtime.createSession(surface, 1080, 720)
        assertEquals(-1L, newId)
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }

    @Test
    fun closeSession_withUnknownId_doesNotCrash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.closeSession(-1L)
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }

    @Test
    fun closeSession_withZeroId_doesNotCrash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.closeSession(0L)
        assertEquals(emptyList<Long>(), runtime.state.value.sessionIds)
    }

    @Test
    fun stop_after_stopThenWrite_doesNotRecreateState() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.stop()
        runtime.writeToPty(byteArrayOf(0x48))
        runtime.stop()
        assertEquals(false, runtime.state.value.isRunning)
    }

    @Test
    fun resize_beforeStart_doesNotCrash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.resize(40, 120)
        assertEquals(24, runtime.state.value.rows)
        assertEquals(80, runtime.state.value.cols)
    }

    @Test
    fun pauseRendering_beforeStart_doesNotCrash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.pauseRendering()
        assertEquals(false, runtime.state.value.isRunning)
    }

    @Test
    fun saveSession_beforeStart_doesNotCrash() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        runtime.saveSession()
        assertEquals(false, runtime.state.value.isRunning)
    }

    @Test
    fun stateFlow_emitsInitialValues() = runTest {
        val context = RuntimeEnvironment.getApplication()
        val provider = SettingsDataStoreProvider(context)
        val settingsRepository = SettingsRepository(provider)
        val runtime = TerminalRuntime(context, settingsRepository)
        val initial = runtime.state.value
        assertEquals(false, initial.isRunning)
        assertEquals(0L, initial.activeSessionId)
        assertEquals(emptyList<Long>(), initial.sessionIds)
    }
}
