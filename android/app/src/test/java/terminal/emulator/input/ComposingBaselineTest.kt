package terminal.emulator.input

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.ui.ComposingDiff

class ComposingBaselineTest {
    @Test
    fun `positive newCursorPosition is an offset from the composing end`() {
        val baseline = ComposingBaseline()
        // 平台语义：正值相对组字末尾。候选区「abc」配 newCursorPosition=1
        // 表示光标越过末尾，钳位到末尾 3。
        assertEquals(3, baseline.resolveCursor("abc", newCursorPosition = 1))
    }

    @Test
    fun `zero newCursorPosition is an absolute position at the composing start`() {
        val baseline = ComposingBaseline()
        assertEquals(0, baseline.resolveCursor("abc", newCursorPosition = 0))
    }

    @Test
    fun `negative newCursorPosition is clamped to the composing start`() {
        val baseline = ComposingBaseline()
        // 负值按绝对位置解释并钳位：-1 落在开头而非末尾。
        assertEquals(0, baseline.resolveCursor("abc", newCursorPosition = -1))
    }

    @Test
    fun `null text ends composing and clears both fields`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abc", cursor = 3)
        baseline.composeAt(text = null, cursor = 1)
        assertEquals("", baseline.text)
        assertEquals(0, baseline.cursor)
        assertFalse(baseline.isComposing)
    }

    @Test
    fun `empty text ends composing`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abc", cursor = 0)
        baseline.composeAt("", cursor = 1)
        assertFalse(baseline.isComposing)
    }

    @Test
    fun `cursor is clamped into the composing text`() {
        val baseline = ComposingBaseline()
        // 输入法给出远超末尾的偏移：钳到末尾而非抛异常。
        baseline.composeAt("ab", cursor = 99)
        assertEquals(2, baseline.cursor)
    }

    @Test
    fun `textBefore and textAfter split the composing text at the cursor`() {
        val baseline = ComposingBaseline()
        // 绝对位置 0 落在组字开头：before 为空，after 从光标起取 length 个。
        baseline.composeAt("abcdef", cursor = 0)
        assertEquals("", baseline.textBefore(3))
        assertEquals("abc", baseline.textAfter(3))
        // 越界取文只给已存在的部分。
        assertEquals("abcdef", baseline.textAfter(99))
    }

    @Test
    fun `non-positive lengths return empty`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abc", cursor = 0)
        assertEquals("", baseline.textBefore(0))
        assertEquals("", baseline.textAfter(-1))
    }

    @Test
    fun `getters reflect only the composing baseline, never screen text`() {
        val baseline = ComposingBaseline()
        // 未组字时取文必须为空：输入法不得把终端屏幕内容当成自己的候选区。
        assertTrue(baseline.textBefore(10).isEmpty())
        assertTrue(baseline.textAfter(10).isEmpty())
        baseline.composeAt("ni", cursor = 0)
        assertEquals("", baseline.textBefore(5))
        assertEquals("ni", baseline.textAfter(5))
    }

    @Test
    fun `deletionTarget removes code points on both sides of the composing cursor`() {
        val baseline = ComposingBaseline()
        val emoji = "😀"
        baseline.composeAt("a${emoji}bc", cursor = 3)
        val target = baseline.deletionTarget(beforeLength = 1, afterLength = 1)
        assertEquals("ac", target.text)
        assertEquals(1, target.cursor)
        assertEquals(1, target.beforeCount)
        assertEquals(1, target.afterCount)
    }

    @Test
    fun `deletionTarget clamps lengths to the composing baseline`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("ab", cursor = 0)
        val target = baseline.deletionTarget(beforeLength = 0, afterLength = 9)
        assertEquals("", target.text)
        assertEquals(0, target.cursor)
        assertEquals(2, target.afterCount)
    }

    @Test
    fun `deletionTarget removes surrogate pairs as one code point`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("a😀b", cursor = 1)
        val target = baseline.deletionTarget(beforeLength = 0, afterLength = 1)
        assertEquals("ab", target.text)
        assertEquals(1, target.afterCount)
    }

    @Test
    fun `replaceRange splices head replacement and tail`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abcdef", cursor = 0)
        // 替换 [2,4)="cd" 为 "X"：头 "ab" + "X" + 尾 "ef"。
        assertEquals("abXef", baseline.replaceRange(2, 4, "X"))
    }

    @Test
    fun `replaceRange accepts an empty range as pure insertion`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("ab", cursor = 0)
        assertEquals("aXYb", baseline.replaceRange(1, 1, "XY"))
    }

    @Test
    fun `replaceRange supports non-ascii replacement`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("ni", cursor = 0)
        assertEquals("你", baseline.replaceRange(0, 2, "你"))
    }

    @Test
    fun `replaceRange rejects a range outside the baseline`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("ab", cursor = 0)
        // 越界下标、倒置区间与负起点一律返回 null，调用方据此拒绝且不发字节。
        assertNull(baseline.replaceRange(0, 3, "X"))
        assertNull(baseline.replaceRange(3, 3, "X"))
        assertNull(baseline.replaceRange(-1, 1, "X"))
        assertNull(baseline.replaceRange(2, 1, "X"))
    }

    @Test
    fun `region reanchor accepts only the whole baseline`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abcdef", cursor = 0)
        // 平台语义：重锚只改「哪一段正在组字」，不改文本。本仓的组字基线整体即
        // 组字区，故只有覆盖整段基线的重锚可受理。
        assertTrue(baseline.acceptsRegionReanchor(0, 6))
        // 子区间重锚要求后续组字只替换该子区间，本仓无从处置区间外的已提交内容：
        // 如实拒绝，不发字节。
        assertFalse(baseline.acceptsRegionReanchor(2, 4))
        assertFalse(baseline.acceptsRegionReanchor(3, 3))
    }

    @Test
    fun `region reanchor rejects a range outside the baseline`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("ab", cursor = 0)
        assertFalse(baseline.acceptsRegionReanchor(0, 9))
        assertFalse(baseline.acceptsRegionReanchor(5, 5))
        assertFalse(baseline.acceptsRegionReanchor(-2, 2))
    }

    @Test
    fun `region reanchor on an empty baseline accepts the empty range`() {
        val baseline = ComposingBaseline()
        // 未组字时整段基线为空：空区间即整段，可受理；非空范围无从处置。
        assertTrue(baseline.acceptsRegionReanchor(0, 0))
        assertFalse(baseline.acceptsRegionReanchor(0, 1))
    }

    @Test
    fun `replacedCursor lands after the inserted text`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abcdef", cursor = 6)
        // 平台口径：newCursorPosition=1 落在被插入文本之后（此处 abXY|ef）。
        assertEquals(
            4,
            baseline.replacedCursor(
                start = 2,
                end = 4,
                insertedLength = 2,
                newCursorPosition = 1,
            ),
        )
    }

    @Test
    fun `replacedCursor with a non-positive offset is relative to the insertion start`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abcdef", cursor = 6)
        // 非正数相对被插入文本开头：0 即插入点之前，-1 再往前一格。
        assertEquals(
            2,
            baseline.replacedCursor(
                start = 2,
                end = 4,
                insertedLength = 2,
                newCursorPosition = 0,
            ),
        )
        assertEquals(
            1,
            baseline.replacedCursor(
                start = 2,
                end = 4,
                insertedLength = 2,
                newCursorPosition = -1,
            ),
        )
    }

    @Test
    fun `replacedCursor clamps out-of-range offsets into the replaced text`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abcdef", cursor = 6)
        // 替换后的文本长 6：越界偏移钳到末尾，不抛异常。
        assertEquals(
            6,
            baseline.replacedCursor(
                start = 2,
                end = 4,
                insertedLength = 2,
                newCursorPosition = 99,
            ),
        )
        assertEquals(
            0,
            baseline.replacedCursor(
                start = 2,
                end = 4,
                insertedLength = 2,
                newCursorPosition = -99,
            ),
        )
    }

    @Test
    fun `reconciling a replaceRange result rewrites only what follows the common prefix`() {
        val baseline = ComposingBaseline()
        baseline.composeAt("abcdef", cursor = 0)
        val target = checkNotNull(baseline.replaceRange(2, 4, "X")) { "replaceRange 必须成功" }
        // 与既有差分出口对接：`ComposingDiff` 的既定策略是「最长公共前缀 + 其后重打」，
        // 故前缀 "ab" 之后的 "cdef" 整体退格，再补新的 "Xef"。
        val edit = ComposingDiff.reconcile(baseline.text, target)
        assertEquals(4, edit.backspaces)
        assertEquals("Xef", edit.append)
    }
}
