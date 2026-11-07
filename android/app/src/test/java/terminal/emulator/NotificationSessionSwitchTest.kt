package terminal.emulator

import android.view.Surface
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
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
        val (runtime, switched) = createSwitchRecordingRuntime()
        val viewModel = createTestViewModel(runtime)
        deliverSurface(viewModel)

        viewModel.requestSessionSwitch(sessionId = SWITCH_TARGET_SESSION_ID, freshIntent = true)

        assertTrue("通知点击未切换到目标会话", switched.await(5, TimeUnit.SECONDS))
        coVerify(exactly = 1) { runtime.switchSession(SWITCH_TARGET_SESSION_ID, any(), any(), any()) }
    }

    @Test
    fun freshNotificationWaitsForTheSurface() {
        val (runtime, switched) = createSwitchRecordingRuntime()
        val viewModel = createTestViewModel(runtime)

        // Surface 尚未交付：此刻切不了，但请求不得丢弃。
        viewModel.requestSessionSwitch(sessionId = SWITCH_TARGET_SESSION_ID, freshIntent = true)
        coVerify(exactly = 0) { runtime.switchSession(any(), any(), any(), any()) }

        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.startRuntime(surface, 1080, 2400)

        assertTrue("Surface 交付后未补切到目标会话", switched.await(5, TimeUnit.SECONDS))
        coVerify(exactly = 1) { runtime.switchSession(SWITCH_TARGET_SESSION_ID, any(), any(), any()) }
    }

    @Test
    fun rebuiltActivityDoesNotReplayTheNotificationSwitch() {
        val runtime = createTestRuntime(sessionIds = listOf(SWITCH_TARGET_SESSION_ID))
        val viewModel = createTestViewModel(runtime)
        deliverSurface(viewModel)

        viewModel.requestSessionSwitch(sessionId = SWITCH_TARGET_SESSION_ID, freshIntent = false)

        coVerify(exactly = 0) { runtime.switchSession(any(), any(), any(), any()) }
    }

    @Test
    fun rebuiltActivityLeavesNoPendingNotificationSwitch() {
        val runtime = createTestRuntime(sessionIds = listOf(SWITCH_TARGET_SESSION_ID))
        val viewModel = createTestViewModel(runtime)

        // Surface 未就绪时的陈旧请求：既不挂起也不切换。
        viewModel.requestSessionSwitch(sessionId = SWITCH_TARGET_SESSION_ID, freshIntent = false)

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
}
