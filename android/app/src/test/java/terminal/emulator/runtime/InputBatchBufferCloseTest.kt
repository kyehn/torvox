package terminal.emulator.runtime

import android.util.Log
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowLog

/**
 * `close()` 的最终刷写必须让写入失败出声。
 *
 * 判别力：执行器把未捕获异常吞掉（任务包装成 FutureTask，异常只进 Future），
 * 写入失败既不落日志也不影响进程——detach 前最后一击键丢失将毫无症状。
 */
@RunWith(RobolectricTestRunner::class)
class InputBatchBufferCloseTest {
    @Test
    fun `close reports the final flush failure`() {
        val buffer =
            InputBatchBuffer.forTest(
                flushSink = { _, _ -> throw IllegalStateException("PTY 写入失败") },
                capacity = 128,
            )
        // 100 字节驻留缓冲（超过组合提交直发上限），只在 close 时刷写。
        buffer.write(ByteArray(100) { 7 })
        buffer.close()

        val deadline = System.currentTimeMillis() + 2_000
        var failures = 0
        while (failures == 0 && System.currentTimeMillis() < deadline) {
            failures = errorLogs().size
            if (failures == 0) Thread.sleep(5)
        }
        assertTrue("close 的最终刷写失败必须记错误日志，实际日志：${errorLogs()}", failures > 0)
    }

    /** 批缓冲的错误级日志。 */
    private fun errorLogs(): List<String> = ShadowLog.getLogsForTag(TAG).filter { it.type == Log.ERROR }.map { it.msg }

    private companion object {
        const val TAG = "InputBatchBuffer"
    }
}
