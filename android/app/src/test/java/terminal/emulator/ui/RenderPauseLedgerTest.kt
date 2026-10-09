package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
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
        val ime = ledger.acquire()
        assertEquals(2, ledger.holderCount)
        // 第二个持有者 MUST NOT 重复暂停：布尔是幂等的，多调只是噪音；
        // 但计数必须仍然分得清，否则先结束的那个会放开仍在等的另一个。
        assertPaused(true)

        ledger.release(ime)
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
            ledger.releaseAndCancel(token)
            assertEquals(0, ledger.holderCount)
            token = ledger.acquire()
            assertEquals(1, ledger.holderCount)
        }
        ledger.release(token)
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
    }

    @Test
    fun `inheriting the cancelled hold keeps the count unchanged`() {
        // 「旧防抖持有、新防抖继承」是连拍调用的实际形态：注销旧的同时不新领，
        // 计数保持 1。本仓两条防抖都走这条。
        val first = ledger.acquire()
        ledger.releaseAndCancel(first)
        // 继承：把注销与领取合成一步时，计数不应出现 0 也不应出现 2。
        val second = ledger.acquire()
        assertEquals(1, ledger.holderCount)
        ledger.release(second)
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
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

    @Test
    fun `reset cancels every hold and makes stale releases no-ops`() {
        val stale = ledger.acquire()
        ledger.reset()
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())

        // reset 之后新防抖领的暂停，不许被 reset 之前的陈旧持有者减掉。
        val fresh = ledger.acquire()
        val before = pauseChanges.size
        ledger.release(stale)
        assertEquals(1, ledger.holderCount)
        assertEquals("陈旧持有者不得发出任何记账变化", before, pauseChanges.size)
        assertTrue("仍在等待的防抖不得被放开", pauseChanges.last())

        ledger.release(fresh)
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
    }

    @Test
    fun `reset with no holders still forces rendering`() {
        // Surface 重建成功的入口必须无条件出一帧，即便此刻没有防抖在等。
        ledger.reset()
        assertEquals(0, ledger.holderCount)
        assertFalse(pauseChanges.last())
    }
}
