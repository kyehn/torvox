package terminal.emulator.util

import org.junit.Assert.assertEquals
import org.junit.Test

class TextTruncationTest {

    @Test
    fun headLeavesShortTextUntouched() {
        assertEquals("hello", truncateTextKeepingHead("hello", maxChars = 10))
    }

    @Test
    fun headKeepsBlankText() {
        // 空白选区是合法的复制/分享内容：截断只裁长度，不把空白折叠成空串。
        assertEquals("   ", truncateTextKeepingHead("   ", maxChars = 10))
    }

    @Test
    fun headCapsAtMaxChars() {
        assertEquals("0123456789", truncateTextKeepingHead("0123456789ABCDEF", maxChars = 10))
    }

    @Test
    fun headNeverSplitsSurrogatePair() {
        // maxChars 恰切在表情中间（高代理项在末尾）时多取一字符保住完整码点。
        assertEquals("ab😀", truncateTextKeepingHead("ab😀c", maxChars = 3))
    }

    @Test
    fun tailLeavesShortTextUntouched() {
        assertEquals("hello", truncateTextKeepingTail("hello", maxChars = 10))
    }

    @Test
    fun tailKeepsTheLastChars() {
        // 复制超长选区只取后段：段首（最旧的输出）被截掉，段尾原样保留。
        assertEquals("bc😀", truncateTextKeepingTail("abc😀", maxChars = 4))
    }

    @Test
    fun tailStartsOnThePairWhenTheBoundaryLandsOnIt() {
        // 起点恰好落在完整码点之前：不多取，也不裁掉字符。
        assertEquals("😀c", truncateTextKeepingTail("ab😀c", maxChars = 3))
    }

    @Test
    fun tailNeverStartsWithaLoneLowSurrogate() {
        // 起点落在低代理项上：它的高代理项已被截掉，把起点前移一格把整对收回。
        val truncated = truncateTextKeepingTail("a😀bc", maxChars = 3)
        assertEquals("😀bc", truncated)
        assertEquals(4, truncated.length)
    }

    @Test
    fun zeroLimitYieldsEmptyText() {
        assertEquals("", truncateTextKeepingHead("abc", maxChars = 0))
        assertEquals("", truncateTextKeepingTail("abc", maxChars = 0))
    }
}
