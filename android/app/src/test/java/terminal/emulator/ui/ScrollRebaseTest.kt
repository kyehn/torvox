package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 滚动偏移重基单元测试：内容在底边增长后，偏移必须同步增长才仍指向同一处内容。
 *
 * 真值口径是「离实时视图多少行」：浏览回滚期间新输出把活动区底边推远，同一处内容
 * 离底边更远了。偏移不跟着长就会漂移——回到底边时停在底边之上（新输出不可见），
 * 滚到回滚区顶端时多滚出实际可滚的行数。
 */
class ScrollRebaseTest {

    @Test
    fun `content growth shifts the offset by the same amount`() {
        // 浏览中停在偏移 10（离底边 10 行），随后新输出让回滚区多出 5 行：
        // 同一处内容离底边远了 5 行，偏移同步 +5 才仍指向它。
        assertEquals(15, rebasedScrollOffset(10, 105, 100))
    }

    @Test
    fun `offset at the bottom stays zero`() {
        // 贴底时视口随新输出下移，偏移恒为 0（0 = 视口贴底）。
        assertEquals(0, rebasedScrollOffset(0, 105, 100))
    }

    @Test
    fun `no growth leaves the offset untouched`() {
        assertEquals(10, rebasedScrollOffset(10, 100, 100))
    }

    @Test
    fun `content shrinkage clamps the offset at the top`() {
        // clear / RIS 让回滚区缩短（甚至归零）：偏移跟着缩，且绝不落到负数。
        assertEquals(4, rebasedScrollOffset(10, 94, 100))
        assertEquals(0, rebasedScrollOffset(10, 0, 100))
    }

    @Test
    fun `rebased offset never exceeds the scrollback length`() {
        // 顶到回滚区顶端：偏移不得超过当前回滚长度，否则会多滚出实际可滚的行数。
        assertEquals(100, rebasedScrollOffset(98, 100, 95))
    }

    @Test
    fun `the requested step survives the rebase`() {
        // 判别力：直接抄原生真值会把尚未落地的那一步抹掉（滚动丢步、手感发黏）。
        // 重基只挪基线：用户已请求的 10 行原样保留，再叠加内容增长的 5 行。
        val requested = 10
        val growth = 5
        val rebased = rebasedScrollOffset(requested, 100 + growth, 100)
        assertEquals(requested + growth, rebased)
    }
}
