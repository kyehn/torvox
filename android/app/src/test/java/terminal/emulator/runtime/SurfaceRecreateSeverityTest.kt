package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [TerminalRuntime] 请求换新原生窗口的日志分级。
 *
 * 判别力：平台收走原生窗口后宿主要换新窗口重挂载，这是自愈路径的正常一步，
 * 按 warn 记会让每次切后台都刷一条 W；连续换不来才说明新窗口迟迟不来，
 * 那一档必须留在 warn。
 *
 * 计数口径是「含本次在内」：预算是先加后判，故首次为 1。
 */
class SurfaceRecreateSeverityTest {

    @Test
    fun `first recreate request is a normal degradation`() {
        assertFalse(surfaceRecreateIsWarning(attempt = 1))
    }

    @Test
    fun `repeated recreate requests stay a warning`() {
        assertTrue(surfaceRecreateIsWarning(attempt = 2))
        assertTrue(surfaceRecreateIsWarning(attempt = 6))
    }
}
