package terminal.emulator.runtime

import kotlinx.coroutines.CancellationException
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 渲染线程的中断信号与渲染失败的区分。
 *
 * 判别力：关闭路径中断渲染线程时，`runBlocking` 以取消异常收尾，其 cause 是
 * `InterruptedException`。把它当渲染失败会记出 error 加完整栈——那是关闭流程的
 * 正常一步；而真正的渲染异常（cause 不是中断）必须继续走 error 路径。
 */
class RenderThreadInterruptionTest {

    @Test
    fun `interrupted exception is a shutdown signal`() {
        assertTrue(isRenderThreadInterruption(InterruptedException("closed")))
    }

    @Test
    fun `cancellation caused by interruption is a shutdown signal`() {
        val cancellation = CancellationException("BlockingCoroutine is cancelling")
        cancellation.initCause(InterruptedException("closed"))
        assertTrue(isRenderThreadInterruption(cancellation))
    }

    @Test
    fun `cancellation without interruption cause stays a render failure`() {
        assertFalse(isRenderThreadInterruption(CancellationException("scope cancelled")))
    }

    @Test
    fun `ordinary render failure stays a render failure`() {
        assertFalse(isRenderThreadInterruption(IllegalStateException("native bridge gone")))
    }
}
