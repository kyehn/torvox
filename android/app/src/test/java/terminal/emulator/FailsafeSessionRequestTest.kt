package terminal.emulator

import android.content.Context
import android.view.Surface
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.runtime.RuntimeState
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxConfiguration
import terminal.emulator.settings.TermuxPropertiesRepository
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * 外部 Failsafe Intent（应用快捷方式 / 第三方启动器）的处置。
 *
 * 两层验证：
 * - [decideFailsafeIntent] 的纯决策（表驱动）；
 * - `TerminalViewModel.requestFailsafeSession` 的接线：标记运行期、并在 Surface
 *   交付后建出会话。
 *
 * 语义（对应 `app-shortcuts` 规格「应用已启动和未启动时都能新建 Failsafe 会话」）：
 * - 应用未启动：只置标记，首个会话由 `runtime.start()` 以 `/system/bin/sh` 建出；
 * - 应用已启动且 Intent 新投递：置标记**并**建一个新会话（`start()` 会因已有会话早退，
 *   只置标记等于点了快捷方式毫无反应）；
 * - Activity 重建时重复投递的同一个启动 Intent：完全忽略——既不建会话（否则每次
 *   配置变更都多出一个应急会话），也不置标记（标记滞留会把无关会话变成应急会话）。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class FailsafeSessionRequestTest {

    @Before
    fun setUp() {
        Dispatchers.setMain(StandardTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    // ── 纯决策 ──────────────────────────────────────────────────────────────

    @Test
    fun coldStartOnlyMarksTheRequest() {
        assertEquals(
            FailsafeIntentAction.MARK_ONLY,
            decideFailsafeIntent(hasSession = false, freshIntent = true),
        )
    }

    @Test
    fun coldStartAfterProcessDeathStillMarksTheRequest() {
        // 进程被杀后系统重建 Activity：会话已随进程消失，首个会话仍应是应急会话。
        assertEquals(
            FailsafeIntentAction.MARK_ONLY,
            decideFailsafeIntent(hasSession = false, freshIntent = false),
        )
    }

    @Test
    fun runningAppCreatesANewSession() {
        assertEquals(
            FailsafeIntentAction.MARK_AND_CREATE,
            decideFailsafeIntent(hasSession = true, freshIntent = true),
        )
    }

    @Test
    fun rebuiltActivityIgnoresTheRepeatedLaunchIntent() {
        assertEquals(
            FailsafeIntentAction.IGNORE,
            decideFailsafeIntent(hasSession = true, freshIntent = false),
        )
    }

    // ── ViewModel 接线 ───────────────────────────────────────────────────────

    @Test
    fun coldStartMarksTheRuntimeWithoutCreatingASession() {
        val runtime = createRuntime(sessionIds = emptyList())
        val viewModel = createViewModel(runtime)

        viewModel.requestFailsafeSession(freshIntent = true)

        verify(exactly = 1) { runtime.requestFailsafeSession() }
        coVerify(exactly = 0) { runtime.createSession(any(), any(), any()) }
    }

    @Test
    fun runningAppCreatesAFailsafeSessionOnceTheSurfaceArrives() {
        val runtime = createRuntime(sessionIds = listOf(7L))
        val created = CountDownLatch(1)
        coEvery { runtime.createSession(any(), any(), any()) } coAnswers {
            created.countDown()
            8L
        }
        val viewModel = createViewModel(runtime)

        viewModel.requestFailsafeSession(freshIntent = true)

        verify(exactly = 1) { runtime.requestFailsafeSession() }
        // Surface 尚未交付：此刻建不出会话，但请求不得丢弃。
        coVerify(exactly = 0) { runtime.createSession(any(), any(), any()) }

        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.surfaceWidth = 1080
        viewModel.surfaceHeight = 2400
        viewModel.currentSurface = surface

        assertTrue("Failsafe 会话未建出", created.await(5, TimeUnit.SECONDS))
        coVerify(exactly = 1) { runtime.createSession(surface, 1080, 2400) }
    }

    @Test
    fun rebuiltActivityNeitherMarksNorCreates() {
        val runtime = createRuntime(sessionIds = listOf(7L))
        val viewModel = createViewModel(runtime)

        viewModel.requestFailsafeSession(freshIntent = false)

        verify(exactly = 0) { runtime.requestFailsafeSession() }
        coVerify(exactly = 0) { runtime.createSession(any(), any(), any()) }
    }

    private fun createRuntime(sessionIds: List<Long>): TerminalRuntime {
        val runtime = mockk<TerminalRuntime>(relaxed = true)
        every { runtime.state } returns
            MutableStateFlow(
                RuntimeState(isRunning = sessionIds.isNotEmpty(), sessionIds = sessionIds),
            )
        return runtime
    }

    private fun createViewModel(runtime: TerminalRuntime): TerminalViewModel {
        val settingsRepository = mockk<SettingsRepository>(relaxed = true)
        every { settingsRepository.settings } returns
            flowOf(
                SettingsRepository.SettingsState(
                    fontSize = 14f,
                    themeName = "",
                    dayThemeName = "",
                    nightThemeName = "",
                ),
            )
        every { settingsRepository.defaultFontSize } returns 14f
        val termuxProperties = mockk<TermuxPropertiesRepository>(relaxed = true)
        every { termuxProperties.configuration } returns MutableStateFlow(TermuxConfiguration())
        return TerminalViewModel(
            context = mockk<Context>(relaxed = true),
            settingsRepository = settingsRepository,
            termuxProperties = termuxProperties,
            runtime = runtime,
        )
    }
}
