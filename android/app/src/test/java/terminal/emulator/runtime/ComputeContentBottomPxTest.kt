package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test
import terminal.emulator.bridge.Bridge

/**
 * 内容下沿必须先浮点乘后取整：原生行顶为 `row * cellHeight`（浮点），
 * 先 `toInt` 再乘会每行丢掉小数并随行数累积，位移偏小、末行被键栏吞掉
 * 且随内容增多扩大（N 行累积误差 = N × 小数部分）。
 */
class ComputeContentBottomPxTest {

    @Test
    fun `empty viewport is zero`() {
        assertEquals(0, computeContentBottomPx(Bridge.LAST_CONTENT_ROW_NONE, 45f))
    }

    @Test
    fun `non-positive cell height is zero`() {
        assertEquals(0, computeContentBottomPx(10, 0f))
        assertEquals(0, computeContentBottomPx(10, -1f))
    }

    @Test
    fun `integer cell height multiplies exactly`() {
        assertEquals(45, computeContentBottomPx(0, 45f))
        assertEquals(450, computeContentBottomPx(9, 45f))
    }

    @Test
    fun `fractional cell height does not accumulate truncation error`() {
        // 50 行 × 45.6px：旧写法先 toInt 再乘得 50×45=2250，少 30px（约一行被吞）；
        // 正确值 ceil(50×45.6)=2280。
        assertEquals(2280, computeContentBottomPx(49, 45.6f))
        // 10 行 × 44.4px：旧写法得 440，正确值 ceil(444.0)=444。
        assertEquals(444, computeContentBottomPx(9, 44.4f))
    }

    @Test
    fun `fractional remainder rounds up not down`() {
        // ceil(10.1)=11：下沿不被低估，至多多移 <1px。
        assertEquals(11, computeContentBottomPx(0, 10.1f))
    }
}
