package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.settings.BellCharacter

/**
 * 响铃执行器的三态与限频。动作以 lambda 注入，不需要 Robolectric：
 * 断言只关心「什么模式触发什么动作、洪泛时动作被限到几次」。
 */
class BellPlayerTest {
    /** 纳秒时钟的手动档：限频窗口完全由测试推进，不依赖真实时间。 */
    private class Clock {
        var now = 0L

        fun advance(nanos: Long) {
            now += nanos
        }
    }

    @Test
    fun `vibrate triggers vibration only`() {
        val clock = Clock()
        var vibrations = 0
        var beeps = 0
        val player =
            BellPlayer(
                vibrateAction = { vibrations++ },
                beepAction = { beeps++ },
                nowNanos = { clock.now },
            )
        player.ring(BellCharacter.VIBRATE)
        assertEquals(1, vibrations)
        assertEquals(0, beeps)
    }

    @Test
    fun `beep triggers tone only`() {
        val clock = Clock()
        var vibrations = 0
        var beeps = 0
        val player =
            BellPlayer(
                vibrateAction = { vibrations++ },
                beepAction = { beeps++ },
                nowNanos = { clock.now },
            )
        player.ring(BellCharacter.BEEP)
        assertEquals(0, vibrations)
        assertEquals(1, beeps)
    }

    @Test
    fun `ignore does nothing at all`() {
        val clock = Clock()
        var actions = 0
        val player =
            BellPlayer(
                vibrateAction = { actions++ },
                beepAction = { actions++ },
                nowNanos = { clock.now },
            )
        player.ring(BellCharacter.IGNORE)
        assertEquals(0, actions)
    }

    /** 限频窗口内的重复 BEL 静默丢弃：`yes '\a'` 一秒上万次，动作至多窗口一次。 */
    @Test
    fun `bell flood inside the rate window is collapsed`() {
        val clock = Clock()
        var actions = 0
        val player =
            BellPlayer(
                vibrateAction = { actions++ },
                beepAction = { actions++ },
                nowNanos = { clock.now },
            )
        player.ring(BellCharacter.VIBRATE)
        // 149ms < MIN_INTERVAL_NANOS(150ms)：窗口内连续 1000 次，全部折叠。
        repeat(1000) {
            clock.advance(149_000_000L / 1000)
            player.ring(BellCharacter.VIBRATE)
        }
        assertEquals(1, actions)
    }

    /** 窗口过后恢复响应：限频是速率约束而不是「只响第一次」。 */
    @Test
    fun `bell after the window elapses rings again`() {
        val clock = Clock()
        var actions = 0
        val player =
            BellPlayer(
                vibrateAction = { actions++ },
                beepAction = { actions++ },
                nowNanos = { clock.now },
            )
        player.ring(BellCharacter.VIBRATE)
        clock.advance(150_000_000L)
        player.ring(BellCharacter.VIBRATE)
        assertEquals(2, actions)
    }

    /** 限频对两种模式一致：beep 换成 vibrate 再立刻响，同样被折叠。 */
    @Test
    fun `rate window applies across modes`() {
        val clock = Clock()
        var actions = 0
        val player =
            BellPlayer(
                vibrateAction = { actions++ },
                beepAction = { actions++ },
                nowNanos = { clock.now },
            )
        player.ring(BellCharacter.BEEP)
        player.ring(BellCharacter.VIBRATE)
        assertTrue("窗口内的第二次响铃必须被折叠，动作总数应为 1：$actions", actions == 1)
    }
}
