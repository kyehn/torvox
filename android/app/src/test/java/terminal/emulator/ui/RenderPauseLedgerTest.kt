package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * [RenderPauseLedger] 的记账不变量。
 *
 * 覆盖两个防抖窗口重叠、取消与强制恢复的全部组合：任何漏归还或陈旧归还都表现为
 * 共享渲染器被**永久**暂停（终端全黑），而这条路径此前只有 View 层的
 * `postDelayed`/`removeCallbacks`，JVM 单测根本够不着。
 */
class RenderPauseLedgerTest {

    /** 记账只应发出「暂停」与「恢复」两拍，中间不重复。 */
    private val pauseChanges = mutableListOf<Boolean>()
    private val ledger = RenderPauseLedger { pauseChanges += it }

    private fun assertPaused(expected: Boolean) = assertEquals(listOf(expected), pauseChanges)

    private fun assertStillPaused() = assertEquals(listOf(true), pauseChanges)

    @Test
    fun `first acquire pauses and final release resumes`() {
        val token = ledger.acquire()
        assertEquals(1, ledger.holderCount)
        assertPaused(true)

        ledger.release(token)
        assertEquals(0, ledger.holderCount)
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `overlapping debounces pause once and resume once`() {
        val surface = ledger.acquire()
        val imePause = ledger.acquire()
        assertEquals(2, ledger.holderCount)
        // 第二个持有者 MUST NOT 重复暂停：布尔是幂等的，多调只是噪音；
        // 但计数必须仍然分得清，否则先结束的那个会放开仍在等的另一个。
        assertPaused(true)

        ledger.release(imePause)
        assertEquals(1, ledger.holderCount)
        assertStillPaused()

        ledger.release(surface)
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
    }

    @Test
    fun `repeated debounce replacement never grows the holder count`() {
        // 键盘弹出动画期间平台逐帧派发 insets，同一个防抖被替换数十次。
        // 每次替换都是「注销旧的 + 领新的」：旧 runnable 永不执行，它的 release
        // 也永不发生，故必须由注销来归还。若实现改成无条件再领一次，计数会单调
        // 增长到不为 0，`setRenderPaused(false)` 再不被调用——终端全黑。
        var token = ledger.acquire()
        repeat(64) {
            ledger.release(token)
            assertEquals(0, ledger.holderCount)
            token = ledger.acquire()
            assertEquals(1, ledger.holderCount)
        }
        ledger.release(token)
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
    }

    @Test
    fun `replacing a hold emits no pause change`() {
        // 连拍调用的实际形态：领新的 → 归还旧的。持有者数走 1 → 2 → 1，全程不落到
        // 0，故 MUST NOT 发出任何恢复/暂停回调。顺序若反过来（先归还后领取），
        // 持有者数会经过 0，每个替换帧都多出一对 setRenderPaused 调用。
        val first = ledger.acquire()
        val second = ledger.acquire()
        ledger.release(first)
        assertEquals(listOf(true), pauseChanges)
        ledger.release(second)
        assertEquals(listOf(true, false), pauseChanges)
    }

    @Test
    fun `replacing the only hold with an unpaused one resumes exactly once`() {
        // 防抖窗内备用屏状态翻转：旧的持一次而新的不持，持有者数 1 → 1 → 0，
        // 恰好发一次恢复。
        val first = ledger.acquire()
        ledger.release(first)
        assertEquals(listOf(true, false), pauseChanges)
        assertEquals(0, ledger.holderCount)
    }

    @Test
    fun `release of an unknown token is a no-op`() {
        ledger.acquire()
        ledger.release(999L)
        assertEquals(1, ledger.holderCount)
        assertPaused(true)
    }

    @Test
    fun `double release does not steal another holder's pause`() {
        val first = ledger.acquire()
        val second = ledger.acquire()
        ledger.release(first)
        ledger.release(first)
        // 第二次归还是陈旧的：若它生效，计数会减到 0 并放开仍在等的 second。
        assertEquals(1, ledger.holderCount)
        assertStillPaused()

        ledger.release(second)
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
    }
}
