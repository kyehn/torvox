package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [TerminalRuntime] 瞬时渲染错误的日志分级。
 *
 * 判别力：首次瞬时错误几乎都是平台收走原生窗口后的正常退化（宿主换新窗口即恢复），
 * 按 warn 记会让每次切后台都刷一条 W，占掉 W 闸门；连续出现才说明新窗口迟迟不来，
 * 那一档必须留在 warn。
 */
class TransientRenderErrorSeverityTest {

    @Test
    fun `first transient error is a normal degradation`() {
        assertFalse(transientRenderErrorIsWarning(consecutiveErrors = 0))
    }

    @Test
    fun `repeated transient errors stay a warning`() {
        assertTrue(transientRenderErrorIsWarning(consecutiveErrors = 1))
        assertTrue(transientRenderErrorIsWarning(consecutiveErrors = 60))
    }
}
