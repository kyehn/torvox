package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicReference

/**
 * 批缓冲的线程契约：粘贴分块从 IO 调度器（无 Looper 的线程）经 `pasteSink` 进来。
 *
 * `Choreographer` 依附**调用线程**的 Looper，直接 `getInstance()` 会抛
 * `IllegalStateException`，把整条粘贴协程炸掉。此处钉住「无 Looper 线程写入大块
 * 数据不抛」，即 [InputBatchBuffer] 把帧调度中转回主 Looper 的理由。
 *
 * 故意走真实构造（`useChoreographer = true`）：`forTest` 工厂关掉帧调度，恰好绕过
 * 这条路径，用它写这条用例会恒绿而钉不住任何东西。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class InputBatchBufferThreadingTest {
    @Test
    fun `large write from a thread without a looper is accepted`() {
        val sent = CopyOnWriteArrayList<ByteArray>()
        val buffer = InputBatchBuffer({ _, data -> sent.add(data) }, inputSessionId = { 1L })
        val failure = AtomicReference<Throwable?>(null)
        val writer =
            Thread {
                try {
                    buffer.write(ByteArray(128) { 7 })
                } catch (error: Throwable) {
                    failure.set(error)
                }
            }
        writer.start()
        writer.join(5_000)
        assertNull("无 Looper 线程写入不得抛异常（实际：${failure.get()}）", failure.get())

        buffer.close()
        val deadline = System.currentTimeMillis() + 2_000
        while (sent.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(5)
        assertEquals("关闭时必须排空驻留字节", 1, sent.size)
        assertEquals(128, sent[0].size)
    }
}
