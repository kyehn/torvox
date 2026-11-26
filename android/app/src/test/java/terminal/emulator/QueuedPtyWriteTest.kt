package terminal.emulator

import io.mockk.every
import io.mockk.verify
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/**
 * UI 线程的输入出口必须入批缓冲，不得直写 PTY。
 *
 * 直写在子进程暂不读 stdin 时阻塞（原生排空上限 5s），主线程上足以触发 ANR；
 * 判据是「字节到了 Surface 安装的队列，而没有走 runtime 的直写」。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class QueuedPtyWriteTest {
    @Before
    fun setUp() {
        Dispatchers.setMain(StandardTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun queuedWriteGoesThroughTheSurfaceQueue() {
        val runtime = createTestRuntime(sessionIds = listOf(7L))
        every { runtime.inputTargetSessionId } returns 7L
        val viewModel = createTestViewModel(runtime)
        val writes = mutableListOf<Pair<Long, ByteArray>>()
        viewModel.pasteSink = { sessionId, data -> writes += sessionId to data }

        viewModel.writeToPtyQueued(7L, byteArrayOf('a'.code.toByte()))

        assertEquals("入队次数", 1, writes.size)
        assertEquals("目标会话", 7L, writes[0].first)
        assertArrayEquals("入队字节", byteArrayOf('a'.code.toByte()), writes[0].second)
        verify(exactly = 0) { runtime.writeToPty(any(), any()) }
    }
}
