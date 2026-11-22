package terminal.emulator

import io.mockk.every
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/**
 * 选区手柄的强调色必须随终端主题实时更新。
 *
 * 此前强调色只在会话创建/切换时顺带抄进 UI 状态：改主题不动运行期状态，
 * 手柄因此一直持旧主题的颜色，直到下一次会话变化才自愈。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class SelectionAccentTest {
    @Before
    fun setUp() {
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `accent follows the runtime theme changes`() {
        val accent = MutableStateFlow(0xFF111111.toInt())
        val runtime = createTestRuntime(sessionIds = listOf(1L))
        every { runtime.accentColor } returns accent
        val viewModel = createTestViewModel(runtime)

        assertEquals(0xFF111111.toInt(), viewModel.state.value.selectionAccent)

        accent.value = 0xFF222222.toInt()

        assertEquals(0xFF222222.toInt(), viewModel.state.value.selectionAccent)
    }
}
