package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 切会话对齐判定：一致跳过、查不到对齐。网格查询以 lambda 传入，无需伪造 Bridge。 */
class ShouldAlignGridOnSwitchTest {

    private fun packed(rows: Int, cols: Int): Long = (rows.toLong() shl 32) or cols.toLong()

    @Test
    fun `matching grid skips resize`() {
        assertFalse(shouldAlignGridOnSwitch(24, 80) { packed(24, 80) })
    }

    @Test
    fun `rows differ aligns`() {
        assertTrue(shouldAlignGridOnSwitch(24, 80) { packed(30, 80) })
    }

    @Test
    fun `cols differ aligns`() {
        assertTrue(shouldAlignGridOnSwitch(24, 80) { packed(24, 100) })
    }

    @Test
    fun `zero packed aligns fail-open`() {
        assertTrue(shouldAlignGridOnSwitch(24, 80) { 0L })
    }

    @Test
    fun `throwing query aligns fail-open`() {
        assertTrue(shouldAlignGridOnSwitch(24, 80) { throw RuntimeException("gone") })
    }
}
