package terminal.emulator

import android.content.Context
import android.view.Surface
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
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
 * 桌面通知点击的会话切换请求。
 *
 * 通知点击把应用从后台拉起并切到通知所属的会话；但 Activity 因配置变更重建时会再次
 * 收到**同一个**启动 Intent（`getIntent()` 仍是通知 Intent），据此再切一次会话，
 * 用户在重建前手动切走的会话就被拽了回去。故重建时的重复投递一律不重放切换。
 *
 * 三种情形：
 * - 新投递 + Surface 已就绪：立刻切换；
 * - 新投递 + Surface 未就绪：挂起请求，Surface 交付后补切（请求不丢）；
 * - 重建时的重复投递：不切换，也不留下待补切的请求。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class NotificationSessionSwitchTest {

    @Before
    fun setUp() {
        Dispatchers.setMain(StandardTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun freshNotificationSwitchesToItsSession() {
        val runtime = createRuntime()
        val switched = CountDownLatch(1)
        coEvery { runtime.switchSession(any(), any(), any(), any()) } coAnswers {
            switched.countDown()
            true
        }
        val viewModel = createViewModel(runtime)
        deliverSurface(viewModel)

        viewModel.requestSessionSwitch(sessionId = 7L, freshIntent = true)

        assertTrue("通知点击未切换到目标会话", switched.await(5, TimeUnit.SECONDS))
        coVerify(exactly = 1) { runtime.switchSession(7L, any(), any(), any()) }
    }

    @Test
    fun freshNotificationWaitsForTheSurface() {
        val runtime = createRuntime()
        val switched = CountDownLatch(1)
        coEvery { runtime.switchSession(any(), any(), any(), any()) } coAnswers {
            switched.countDown()
            true
        }
        val viewModel = createViewModel(runtime)

        // Surface 尚未交付：此刻切不了，但请求不得丢弃。
        viewModel.requestSessionSwitch(sessionId = 7L, freshIntent = true)
        coVerify(exactly = 0) { runtime.switchSession(any(), any(), any(), any()) }

        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.startRuntime(surface, 1080, 2400)

        assertTrue("Surface 交付后未补切到目标会话", switched.await(5, TimeUnit.SECONDS))
        coVerify(exactly = 1) { runtime.switchSession(7L, any(), any(), any()) }
    }

    @Test
    fun rebuiltActivityDoesNotReplayTheNotificationSwitch() {
        val runtime = createRuntime()
        val viewModel = createViewModel(runtime)
        deliverSurface(viewModel)

        viewModel.requestSessionSwitch(sessionId = 7L, freshIntent = false)

        coVerify(exactly = 0) { runtime.switchSession(any(), any(), any(), any()) }
    }

    @Test
    fun rebuiltActivityLeavesNoPendingNotificationSwitch() {
        val runtime = createRuntime()
        val viewModel = createViewModel(runtime)

        // Surface 未就绪时的陈旧请求：既不挂起也不切换。
        viewModel.requestSessionSwitch(sessionId = 7L, freshIntent = false)

        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.startRuntime(surface, 1080, 2400)

        coVerify(exactly = 0) { runtime.switchSession(any(), any(), any(), any()) }
    }

    private fun deliverSurface(viewModel: TerminalViewModel) {
        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.surfaceWidth = 1080
        viewModel.surfaceHeight = 2400
        viewModel.currentSurface = surface
    }

    private fun createRuntime(): TerminalRuntime {
        val runtime = mockk<TerminalRuntime>(relaxed = true)
        every { runtime.state } returns
            MutableStateFlow(
                RuntimeState(isRunning = true, sessionIds = listOf(7L)),
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
