package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 开孔状态日志：去重、上屏只记一次、环形容量与序号。
 *
 * 判别力：闪烁取证依赖「状态没变不记、状态变了必记、每份遮罩只标一次上屏」。
 * 只断言条数非零无法区分「重复流水账」与「漏记」。
 */
class HoleStateJournalTest {
    @Test
    fun `identical state is recorded once`() {
        val journal = HoleStateJournal()
        journal.record(false, false)
        journal.record(false, false)

        assertEquals(1, journal.snapshot().size)
    }

    @Test
    fun `painted is recorded once per cover until reset`() {
        val journal = HoleStateJournal()
        journal.record(false, false)
        journal.recordPainted(false, false)
        journal.recordPainted(false, false)

        assertEquals(2, journal.snapshot().size)
        assertTrue("最后一条必须是上屏记录", journal.snapshot().last().painted)

        journal.resetPainted()
        journal.recordPainted(false, false)
        assertEquals("新一份遮罩可以再记一次上屏", 3, journal.snapshot().size)
    }

    @Test
    fun `state change after painted records painted false`() {
        val journal = HoleStateJournal()
        journal.record(true, true)
        journal.recordPainted(true, true)
        journal.record(false, false)

        assertFalse("状态变化后的记录不得继承上一份遮罩的上屏标记", journal.snapshot().last().painted)
    }

    @Test
    fun `ring keeps newest entries while sequence keeps increasing`() {
        val journal = HoleStateJournal(capacity = 2)
        journal.record(false, false)
        journal.record(true, false)
        journal.record(true, true)

        val snapshot = journal.snapshot()
        assertEquals(2, snapshot.size)
        assertEquals(listOf(2L, 3L), snapshot.map { it.sequence })
    }
}
