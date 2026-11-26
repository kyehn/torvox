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
 * 切换会话后的标题序号。
 *
 * 序号按运行期会话表的既定顺序换算（契约见 [RuntimeState.sessionIds]）。界面层的
 * 会话列表由运行期状态流合并而来，切换刚发生时它可能尚未合并目标会话——此时按
 * 界面列表换算会得到「会话 0」这种不存在的编号，并把抽屉里的编号带偏。
 *
 * 判别力：运行期的状态收集器不在本用例里推进（主调度器不推进），界面会话列表因而
 * 始终为空；序号只能来自运行期会话表。修复前这里得到「会话 0」。
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
}
