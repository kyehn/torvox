package terminal.emulator

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.currentTime
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 打开设置页前等待 bridge 出现的等待预算。
 *
 * 判别力：等待原先是「轮询 50 次」——次数隐含「每次间隔恰好 50 毫秒」的前提，负载高
 * 时实际等待远超标称的 2.5 秒。用例把「以时间预算为界」这条语义钉住：预算内就绪即返回，
 * 迟迟不就绪则按预算超时返回，不无限等待。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class AwaitBridgeReadyTest {

    @Test
    fun `returns true without waiting when the bridge is already ready`() = runTest {
        assertTrue(awaitBridgeReady(isReady = { true }))
        assertEquals(0L, currentTime)
    }

    @Test
    fun `returns true when the bridge becomes ready while waiting`() = runTest {
        var polls = 0
        val ready = awaitBridgeReady(isReady = { polls++ >= 2 })
        assertTrue(ready)
        assertEquals(3, polls)
    }

    @Test
    fun `gives up once the time budget is exhausted`() = runTest {
        val ready = awaitBridgeReady(isReady = { false })
        assertFalse(ready)
        assertEquals(2500L, currentTime)
    }
}
