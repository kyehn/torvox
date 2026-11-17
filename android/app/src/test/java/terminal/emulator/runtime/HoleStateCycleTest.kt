package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.runtime.TerminalRuntime.HoleStateEntry

/**
 * [holeStateCycle] 的表驱动单测：开孔状态日志是环形缓冲，差分必须按序号取。
 *
 * 回归背景：差分原先按条数取（`journalAfter.drop(journalBefore.size)`）。日志满了
 * 之后前后条数相等，整段后台往返被算成「新增 0 条」，闪烁用例即以「本次往返必须
 * 产生开孔状态转换记录（实测 0 条）」判红——红灯指向用例的差分口径，与被测的
 * 开孔行为无关。
 */
class HoleStateCycleTest {

    private fun entry(bufferPresent: Boolean, coverPresent: Boolean, painted: Boolean = false, sequence: Long) =
        HoleStateEntry(bufferPresent, coverPresent, painted, sequence)

    @Test
    fun emptyJournalYieldsEmptyCycle() {
        assertTrue(holeStateCycle(emptyList(), emptyList()).isEmpty())
    }

    @Test
    fun onlyEntriesAfterTheSnapshotAreReturned() {
        val before = listOf(entry(true, false, sequence = 1L), entry(false, true, sequence = 2L))
        val after = before + listOf(entry(false, true, painted = true, sequence = 3L))

        val cycle = holeStateCycle(before, after)

        assertEquals(1, cycle.size)
        assertEquals(3L, cycle.single().sequence)
    }

    @Test
    fun wrappedJournalStillYieldsTheNewEntries() {
        // 环形缓冲绕回：往返前后的快照条数相同（都满），内容却不同。
        val before = (1L..32L).map { entry(it % 2 == 0L, it % 2 == 1L, sequence = it) }
        val after = (17L..48L).map { entry(it % 2 == 0L, it % 2 == 1L, sequence = it) }

        val cycle = holeStateCycle(before, after)

        assertEquals(16, cycle.size)
        assertEquals(33L, cycle.first().sequence)
        assertEquals(48L, cycle.last().sequence)
    }

    @Test
    fun emptySnapshotTakesEveryEntryAsNew() {
        val after = listOf(entry(true, false, sequence = 1L), entry(false, true, sequence = 2L))

        assertEquals(2, holeStateCycle(emptyList(), after).size)
    }
}
