package terminal.emulator.runtime

import kotlinx.coroutines.CancellationException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 遮罩回读失败时的兜底。
 *
 * 回读发生在渲染线程的**退出路径**上，所在的外层 try 只接 `InterruptedException`：
 * 这里的任何异常都会逃出渲染线程、掀掉整个进程（真机日志：切后台时遮罩帧回读
 * 撞上设备错误，进程 FATAL 退出并重启）。
 *
 * 判别力：少了兜底时，`readCoverFrame` 抛出的异常会一路逃出渲染线程；
 * 用例断言「失败即返回 null 且上报一次」，以及「取消信号继续上抛」。
 */
class ReadCoverFrameSafelyTest {

    @Test
    fun `returns the frame when the read succeeds`() {
        val frame = "cover"
        var failures = 0
        val result =
            readCoverFrameSafely(
                read = { frame },
                onFailure = { failures++ },
            )
        assertSame(frame, result)
        assertEquals(0, failures)
    }

    @Test
    fun `returns null and reports once when the read throws`() {
        var reported: Exception? = null
        val result =
            readCoverFrameSafely<String>(
                read = { throw IllegalStateException("device lost") },
                onFailure = { reported = it },
            )
        assertNull(result)
        assertEquals("device lost", reported?.message)
    }

    @Test
    fun `a missing frame is not a failure`() {
        var failures = 0
        val result =
            readCoverFrameSafely<String>(
                read = { null },
                onFailure = { failures++ },
            )
        assertNull(result)
        assertEquals("没有可遮罩的内容不是失败，不该上报", 0, failures)
    }

    @Test
    fun `cancellation is rethrown rather than swallowed`() {
        // 取消信号不是回读失败：吞掉它会让协程在取消后继续跑。
        val cancellation = CancellationException("cancelled")
        val thrown =
            assertThrows(CancellationException::class.java) {
                readCoverFrameSafely<String>(
                    read = { throw cancellation },
                    onFailure = { throw AssertionError("取消信号不得走失败上报") },
                )
            }
        assertSame(cancellation, thrown)
        assertTrue(thrown.message!!.contains("cancelled"))
    }
}
