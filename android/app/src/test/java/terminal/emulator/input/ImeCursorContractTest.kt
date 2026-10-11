package terminal.emulator.input

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ImeCursorContractTest {
    // ── BatchEditDepth ─────────────────────────────────────────────

    @Test
    fun `nested batch edits are counted`() {
        val depth = BatchEditDepth()
        assertEquals(0, depth.depth)
        assertFalse(depth.isActive)
        depth.begin()
        depth.begin()
        assertEquals(2, depth.depth)
        assertTrue(depth.isActive)
        depth.end()
        assertEquals(1, depth.depth)
        assertTrue(depth.isActive)
        depth.end()
        assertEquals(0, depth.depth)
        assertFalse(depth.isActive)
    }

    @Test
    fun `surplus endBatchEdit clamps at zero instead of going negative`() {
        val depth = BatchEditDepth()
        depth.begin()
        depth.end()
        depth.end()
        depth.end()
        // 负深度会让随后一次 begin 仍停留在非批量状态，反而错过后续整段编辑。
        assertEquals(0, depth.depth)
        assertFalse(depth.isActive)
        depth.begin()
        assertTrue(depth.isActive)
    }

    // ── SelectionMover ─────────────────────────────────────────────

    @Test
    fun `forward move becomes a rightward delta`() {
        val mover = SelectionMover(maxMagnitude = 4096)
        val delta = mover.moveTo(1)
        assertEquals(1, delta.rightward())
        assertEquals(0, delta.leftward())
        assertFalse(delta.isEmpty)
    }

    @Test
    fun `backward move becomes a leftward delta`() {
        val mover = SelectionMover(maxMagnitude = 4096)
        mover.moveTo(5)
        val delta = mover.moveTo(4)
        assertEquals(0, delta.rightward())
        assertEquals(1, delta.leftward())
    }

    @Test
    fun `oversized displacement is clamped and the record advances by the clamped step`() {
        val mover = SelectionMover(maxMagnitude = 10)
        assertEquals(10, mover.moveTo(9999).rightward())
        // 记录必须按**钳位后**的步长（10）而非目标位置（9999）推进，
        // 否则下一次折算会重复计入被丢掉的部分。
        assertEquals(5, mover.moveTo(15).rightward())
    }

    @Test
    fun `oversized negative displacement is clamped too`() {
        val mover = SelectionMover(maxMagnitude = 10)
        mover.resetTo(9999)
        assertEquals(10, mover.moveTo(0).leftward())
    }

    @Test
    fun `unchanged position yields no movement`() {
        val mover = SelectionMover(maxMagnitude = 4096)
        mover.moveTo(3)
        assertTrue(mover.moveTo(3).isEmpty)
    }

    @Test
    fun `resetTo re-anchors the origin`() {
        val mover = SelectionMover(maxMagnitude = 4096)
        mover.moveTo(7)
        mover.resetTo(2)
        assertEquals(1, mover.moveTo(3).rightward())
    }

    @Test
    fun `magnitude counts input-method positions verbatim`() {
        val mover = SelectionMover(maxMagnitude = 4096)
        // 输入法给的是**它自己**文本里的下标，本层无从得知码点边界，
        // 故折算按其单位原样相减，只对幅度做上限钳位。
        // 调用方（`setSelection`）负责把结果送进既有键事件路径。
        mover.resetTo(2)
        assertEquals(2, mover.moveTo(0).leftward())
    }
}
