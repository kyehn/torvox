package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.bridge.TestBridges

/**
 * 「本会话首个 PTY 输出」只报一次。
 *
 * 判别力：渲染线程重启（切后台、换 surface）会重建循环内的局部标志，
 * 恢复后若再走一次判定，就会重复记 `first output received` 并把
 * startupToFirstOutputMs 覆盖成恢复后的时间。判定必须挂在会话条目上。
 */
class FirstOutputReportTest {

    private fun entry(): SessionEntry = SessionEntry(
        sessionId = 1L,
        bridge = TestBridges.create(),
        renderThreadRef = null,
        running = true,
    )

    @Test
    fun `首个输出只被消费一次`() {
        val entry = entry()

        assertTrue("首个 PTY 输出必须上报", entry.consumeFirstOutput())
        assertFalse("同一会话不得重复上报首个输出", entry.consumeFirstOutput())
    }
}
