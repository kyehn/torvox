package terminal.emulator

import android.view.Surface
import androidx.test.core.app.ApplicationProvider
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import terminal.emulator.runtime.RuntimeState
import terminal.emulator.runtime.TerminalRuntime

/**
 * 新建/切换会话后的标题序号与抽屉会话表。
 *
 * 序号按运行期会话表的既定顺序换算（契约见 [RuntimeState.sessionIds]）。界面层的
 * 会话列表由运行期状态流在主线程合并，而新建/切换在 IO 线程直接改状态：运行期刚
 * 发布的会话表此刻可能还没被合并进界面层列表——按界面层列表换算会得到偏小的序号，
 * 目标不在列表里时甚至是 0（不存在的「会话 0」），按它合并还会把运行期已有而界面
 * 未合并的会话整个丢掉（抽屉里凭空少一个会话）。
 *
 * 判别力：运行期的状态收集器不在本用例里推进（主调度器不推进），界面会话列表因而
 * 始终为空；会话表与序号只能来自运行期会话表。修复前这里得到「会话 0」/「会话 1」，
 * 抽屉里也只剩新建的那一个会话。
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SessionNumberTest {

    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()

    @Before
    fun setUp() {
        Dispatchers.setMain(StandardTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `switch numbers the session from the runtime session table`() {
        val runtime = mockk<TerminalRuntime>(relaxed = true)
        every { runtime.state } returns
            MutableStateFlow(
                RuntimeState(isRunning = true, title = "", sessionIds = listOf(3L, 7L)),
            )
        coEvery { runtime.switchSession(any(), any(), any(), any()) } returns true
        val viewModel = createTestViewModel(runtime, context)

        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.surfaceWidth = 1080
        viewModel.surfaceHeight = 2400
        viewModel.currentSurface = surface

        viewModel.switchSession(7L)

        val switched = runBlocking {
            withTimeout(5_000) { viewModel.state.first { it.activeSessionId == 7L } }
        }
        assertEquals(
            context.getString(R.string.session_number, 2),
            switched.title,
        )
    }

    @Test
    fun `create numbers the session and keeps the runtime session table`() {
        val runtimeState =
            MutableStateFlow(
                RuntimeState(isRunning = true, title = "", sessionIds = listOf(3L, 7L)),
            )
        val runtime = mockk<TerminalRuntime>(relaxed = true)
        every { runtime.state } returns runtimeState
        coEvery { runtime.createSession(any(), any(), any(), any()) } coAnswers {
            // 运行期在返回前已发布新的会话表（见 TerminalRuntime.createSession）。
            runtimeState.update { it.copy(sessionIds = listOf(3L, 7L, 9L)) }
            9L
        }
        val viewModel = createTestViewModel(runtime, context)

        val surface = mockk<Surface>()
        every { surface.isValid } returns true
        viewModel.surfaceWidth = 1080
        viewModel.surfaceHeight = 2400
        viewModel.currentSurface = surface

        viewModel.createSession()

        val created = runBlocking {
            withTimeout(5_000) { viewModel.state.first { it.activeSessionId == 9L } }
        }
        // 序号按运行期会话表换算：新会话排在运行期已有会话之后，是第 3 个。
        assertEquals(
            context.getString(R.string.session_number, 3),
            created.title,
        )
        // 运行期已有而界面层尚未合并的会话不得从抽屉里消失。
        assertEquals(
            listOf(3L, 7L, 9L),
            created.sessions.map { it.sessionId },
        )
    }
}
