package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.bridge.TestBridges
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * 渲染线程的拆卸信号与退出标记。
 *
 * 两处都在「旧线程退场、新线程接管」的交界上出错：中断信号送不到目标线程，
 * join 只能白等满超时；旧线程的退出写入覆盖新线程的存活状态，监视器随即把
 * 健康的渲染线程杀掉重启——GPU 挂起后的恢复路径恰好就走这条路。
 */
class RenderThreadShutdownTest {

    /** 一个待处置的会话条目。 */
    private fun entry(): SessionEntry = SessionEntry(
        sessionId = 1L,
        bridge = TestBridges.create(),
        renderThreadRef = null,
        running = true,
    )

    /**
     * 拆卸路径必须把中断信号真的送到目标线程。
     *
     * 判别力：中断信号若只送到「引用里当前那个线程」，而引用已被换线程的路径
     * 置空，信号便从未送达——本例的等待线程将一直等到超时。
     */
    @Test
    fun `waking the render thread delivers the interrupt signal to it`() {
        val entry = entry()
        val interrupted = CountDownLatch(1)
        val thread =
            Thread {
                try {
                    Thread.sleep(10_000)
                } catch (expected: InterruptedException) {
                    interrupted.countDown()
                }
            }
        thread.start()

        entry.wakeRenderThread(thread)

        assertTrue("拆卸路径必须把中断信号送到目标线程", interrupted.await(2, TimeUnit.SECONDS))
        thread.join(2_000L)
    }

    /** 仍在册的渲染线程退出时必须标记已退出——否则监视器无从知道线程没了。 */
    @Test
    fun `the registered render thread marks itself exited`() {
        val entry = entry()
        val thread = Thread { entry.markRenderThreadExited() }
        entry.renderThreadRef = thread

        thread.start()
        thread.join(2_000L)

        assertTrue("仍在册的渲染线程退出时必须标记已退出", entry.renderThreadExited)
    }

    /**
     * 已被换下的旧线程退出时不得覆盖新线程的存活状态。
     *
     * 判别力：旧线程退出时若无条件置位，新线程刚起就被误标为已死，监视器随即
     * 把它杀掉重启——本例的标志会由假变真。
     */
    @Test
    fun `a replaced render thread must not mark the new one exited`() {
        val entry = entry()
        val oldThread = Thread { entry.markRenderThreadExited() }
        entry.renderThreadRef = oldThread
        // 换线程：新线程接管引用后，旧线程的退出写入不得落到新线程头上。
        entry.renderThreadRef = Thread {}

        oldThread.start()
        oldThread.join(2_000L)

        assertFalse("旧线程退出不得把新线程误标为已死", entry.renderThreadExited)
    }

    /** 没有在册线程时不得置位：引用尚未写入的条目不算「线程已退出」。 */
    @Test
    fun `an entry without a registered thread is not marked exited`() {
        val entry = entry()

        entry.markRenderThreadExited()

        assertFalse("没有在册线程时不得标记已退出", entry.renderThreadExited)
    }
}
