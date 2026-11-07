package terminal.emulator.runtime

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.currentTime
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 启动时等待原生库加载的等待预算。
 *
 * 判别力：等待原先是「轮询 60 次」——次数隐含「每次间隔恰好 50 毫秒」的前提，
 * 负载高时实际等待远超标称的 3 秒，且上限与含义分居两处（次数在调用点、时长在
 * 注释里）。用例把「以时间预算为界」这条语义钉住：预算内完成即返回，装载迟迟
 * 不来则按预算超时返回，不无限等待。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class AwaitNativeLibraryLoadTest {

    @Test
    fun `returns true without waiting when the library is already loaded`() = runTest {
        assertTrue(awaitNativeLibraryLoad(isLoaded = { true }))
        assertEquals(0L, currentTime)
    }

    @Test
    fun `returns true when the library finishes loading while waiting`() = runTest {
        var polls = 0
        val loaded = awaitNativeLibraryLoad(isLoaded = { polls++ >= 3 })
        assertTrue(loaded)
        assertEquals(4, polls)
    }

    @Test
    fun `gives up once the time budget is exhausted`() = runTest {
        val loaded = awaitNativeLibraryLoad(isLoaded = { false })
        assertFalse(loaded)
        assertEquals(3000L, currentTime)
    }
}
