package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * [TerminalRuntime] 渲染失败后的退避档位。
 *
 * 判别力：阈值原先是内联字面量 10，退避时长与阈值分居两处（一个在调用点、一个在
 * 另一常量的注释里），改动其一不会带动另一处。用例把两者的对应关系钉在同一处：
 * 短退避覆盖到阈值本身，长退避从越过阈值的第一条开始。
 */
class RenderErrorBackoffTest {

    @Test
    fun `short backoff covers the errors up to the threshold`() {
        assertEquals(50L, renderErrorBackoffMillis(consecutiveErrors = 1))
        assertEquals(50L, renderErrorBackoffMillis(consecutiveErrors = 10))
    }

    @Test
    fun `long backoff starts right after the threshold`() {
        assertEquals(200L, renderErrorBackoffMillis(consecutiveErrors = 11))
        assertEquals(200L, renderErrorBackoffMillis(consecutiveErrors = 60))
    }

    @Test
    fun `no error yet still uses the short backoff`() {
        assertEquals(50L, renderErrorBackoffMillis(consecutiveErrors = 0))
    }
}
