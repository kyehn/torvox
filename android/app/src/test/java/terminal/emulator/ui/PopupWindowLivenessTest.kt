package terminal.emulator.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 弹窗存活判据的状态机：已 show、首帧遍历未完成（「pending」）与已挂上窗口
 * （「attached」）都算存活，平台把窗口收走（detach）后不算。
 */
class PopupWindowLivenessTest {

    @Test
    fun `new liveness is not alive`() {
        assertFalse(PopupWindowLiveness().alive)
    }

    @Test
    fun `shown window is alive before the content view attaches`() {
        val liveness = PopupWindowLiveness()
        liveness.markShown()
        assertTrue(liveness.alive)
    }

    @Test
    fun `attached window stays alive`() {
        val liveness = PopupWindowLiveness()
        liveness.markShown()
        liveness.markAttached()
        assertTrue(liveness.alive)
    }

    @Test
    fun `platform removal kills the window`() {
        val liveness = PopupWindowLiveness()
        liveness.markShown()
        liveness.markAttached()
        liveness.markDetached()
        assertFalse(liveness.alive)
    }

    @Test
    fun `shown window that never attached is dead after detach`() {
        val liveness = PopupWindowLiveness()
        liveness.markShown()
        liveness.markDetached()
        assertFalse(liveness.alive)
    }
}
