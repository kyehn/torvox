package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * vsync 帧回调链的停链判据。
 *
 * 链常驻的代价是主线程按显示帧率被唤醒（120Hz 屏即每秒 120 次），而空闲期它什么
 * 也不产出：停链后渲染线程落在 500ms 空闲闭锁上，那本就是空闲期的既定节拍。
 * 反过来，链停着而会话仍在活跃（输入、滚动、新输出）就会丢掉 vsync 对齐——
 * 渲染只剩 17ms 闭锁这一路节拍。
 */
class VsyncChainStopTest {

    @Test
    fun `chain keeps running while a session is active in the foreground`() {
        assertFalse(
            "前台有活跃会话时链必须继续",
            shouldStopVsyncChain(
                hasSessions = true,
                appForeground = true,
                hasActiveSession = true,
            ),
        )
    }

    @Test
    fun `chain stops when the last session is closed`() {
        assertTrue(
            "关闭最后一个会话后链必须停止",
            shouldStopVsyncChain(
                hasSessions = false,
                appForeground = true,
                hasActiveSession = false,
            ),
        )
    }

    @Test
    fun `chain stops while the app is not visible`() {
        assertTrue(
            "应用不可见时链必须停止",
            shouldStopVsyncChain(
                hasSessions = true,
                appForeground = false,
                hasActiveSession = true,
            ),
        )
    }

    @Test
    fun `chain stops when every session is idle`() {
        assertTrue(
            "全部会话空闲时链必须停止",
            shouldStopVsyncChain(
                hasSessions = true,
                appForeground = true,
                hasActiveSession = false,
            ),
        )
    }

    @Test
    fun `chain restarts when a signal arrives between the stop decision and the stop`() {
        assertTrue(
            "判定时空闲、复核时活跃说明新信号刚到，必须重新拉起链",
            shouldRestartVsyncChain(
                hasSessions = true,
                appForeground = true,
                wasActive = false,
                isActiveNow = true,
            ),
        )
    }

    @Test
    fun `chain is not restarted while the app is not visible`() {
        assertFalse(
            "应用不可见时复核必然得到会话仍活跃，据此重拉起会让链每显示帧停一次又起一次",
            shouldRestartVsyncChain(
                hasSessions = true,
                appForeground = false,
                wasActive = true,
                isActiveNow = true,
            ),
        )
    }

    @Test
    fun `chain is not restarted when there is no session`() {
        assertFalse(
            "已无会话时不得重新拉起链",
            shouldRestartVsyncChain(
                hasSessions = false,
                appForeground = true,
                wasActive = false,
                isActiveNow = true,
            ),
        )
    }

    @Test
    fun `chain is not restarted while the session stays idle`() {
        assertFalse(
            "会话持续空闲时不得重新拉起链",
            shouldRestartVsyncChain(
                hasSessions = true,
                appForeground = true,
                wasActive = false,
                isActiveNow = false,
            ),
        )
    }
}
