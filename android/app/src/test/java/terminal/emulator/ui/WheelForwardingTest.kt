package terminal.emulator.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 触摸拖动转发为滚轮时的「已转发」标记单元测试。
 *
 * 标记决定本地回滚是否接管这次手势：一旦转发过，整段手势（含惯性段）都属于远端。
 */
class WheelForwardingTest {

    @Test
    fun `not forwarded before and not encoded now stays not forwarded`() {
        assertFalse(wheelForwardedState(forwardedBefore = false, encodedNow = false))
    }

    @Test
    fun `first successful encoding marks the gesture as forwarded`() {
        assertTrue(wheelForwardedState(forwardedBefore = false, encodedNow = true))
    }

    @Test
    fun `forwarded gesture stays forwarded when a later encoding fails`() {
        // VT 忙时编码拿不到编码器（查询超时）返回空：此前已转发的档位仍属远端，
        // 本地回滚不得接管——否则应用与本地同时滚动。
        assertTrue(wheelForwardedState(forwardedBefore = true, encodedNow = false))
    }

    @Test
    fun `forwarded gesture stays forwarded when a later encoding succeeds`() {
        assertTrue(wheelForwardedState(forwardedBefore = true, encodedNow = true))
    }
}
