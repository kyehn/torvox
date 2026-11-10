package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [BackgroundRenderPause] 的持有不变量。
 *
 * 「切后台」那次暂停只能由「回到前台」这一转场解除。渲染路径上的恢复事件
 * （Surface 重新交付、尺寸防抖到期）可能在应用仍处于后台时到达——那条路径若能
 * 代还，渲染线程就重新在后台对着正在消亡的 BufferQueue 取帧
 * （`ERROR_SURFACE_LOST_KHR`，切回前台后终端永久黑屏）。
 *
 * 凭证由本类私有持有：别处拿不到也还不掉，故「归还了别人的持有」这一类事故
 * 在结构上就不可能发生——本组用例钉住的是它对外暴露的行为。
 */
class BackgroundRenderPauseTest {

    private val pauseChanges = mutableListOf<Boolean>()
    private val ledger = RenderPauseLedger { pauseChanges += it }
    private val backgroundPause = BackgroundRenderPause(ledger)

    @Test
    fun `background acquires one hold and foreground releases it`() {
        backgroundPause.onBackground()
        assertTrue(backgroundPause.isPaused)
        assertEquals(1, ledger.holderCount)
        assertEquals(listOf(true), pauseChanges)

        backgroundPause.onForeground()
        assertFalse(backgroundPause.isPaused)
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `foreground without a background hold is a no-op`() {
        backgroundPause.onForeground()
        assertEquals(0, ledger.holderCount)
        assertTrue(pauseChanges.isEmpty())
    }

    @Test
    fun `repeated background does not grow the holder count`() {
        // 未回到前台又切一次后台：先领新的再归还旧的，持有者数全程为 1，
        // 不得经过 0（那会多发一对恢复/暂停）。
        backgroundPause.onBackground()
        backgroundPause.onBackground()
        assertEquals(1, ledger.holderCount)
        assertEquals(listOf(true), pauseChanges)

        backgroundPause.onForeground()
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `repeated foreground releases the hold only once`() {
        backgroundPause.onBackground()
        backgroundPause.onForeground()
        backgroundPause.onForeground()
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `another holder keeps the renderer paused after the foreground transition`() {
        // 设置页仍在展示时它自己持一次暂停：回到前台归还后台那次之后，
        // 计数仍非零，渲染保持暂停。
        val settingsHold = ledger.acquire()
        backgroundPause.onBackground()
        backgroundPause.onForeground()
        assertEquals(1, ledger.holderCount)
        assertEquals(listOf(true), pauseChanges)

        ledger.release(settingsHold)
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }
}
