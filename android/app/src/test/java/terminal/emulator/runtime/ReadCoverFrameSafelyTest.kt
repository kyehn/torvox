package terminal.emulator.runtime

import kotlinx.coroutines.CancellationException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * 遮罩回读失败时的兜底。
 *
 * 回读发生在 IO 线程的周期回读循环里：那里的异常会终止回读协程，此后遮罩再也不会
 * 更新。（回读曾落在渲染线程的退出路径上，真机日志里那次设备错误掀掉了整个进程；
 * 回读已移出该路径，但兜底仍须保留——回读失败不能拖垮整条循环。）
 *
 * 判别力：少了兜底时，`readCoverFrame` 抛出的异常会一路逃出回读协程；
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
        assertEquals("cancelled", thrown.message)
    }
}
