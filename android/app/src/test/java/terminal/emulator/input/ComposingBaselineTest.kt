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
        baseline.compose("abc", newCursorPosition = 1)
        assertEquals(3, baseline.cursor)
    }

    @Test
    fun `zero newCursorPosition is an absolute position at the composing start`() {
        val baseline = ComposingBaseline()
        baseline.compose("abc", newCursorPosition = 0)
        assertEquals(0, baseline.cursor)
    }

    @Test
    fun `negative newCursorPosition is clamped to the composing start`() {
        val baseline = ComposingBaseline()
        // 负值按绝对位置解释并钳位：-1 落在开头而非末尾。
        baseline.compose("abc", newCursorPosition = -1)
        assertEquals(0, baseline.cursor)
    }

    @Test
    fun `null text ends composing and clears both fields`() {
        val baseline = ComposingBaseline()
        baseline.compose("abc", newCursorPosition = 0)
        baseline.compose(text = null, newCursorPosition = 1)
        assertEquals("", baseline.text)
        assertEquals(0, baseline.cursor)
        assertFalse(baseline.isComposing)
    }

    @Test
    fun `empty text ends composing`() {
        val baseline = ComposingBaseline()
        baseline.compose("abc", newCursorPosition = 0)
        baseline.compose("", newCursorPosition = 1)
        assertFalse(baseline.isComposing)
    }

    @Test
    fun `cursor is clamped into the composing text`() {
        val baseline = ComposingBaseline()
        // 输入法给出远超末尾的偏移：钳到末尾而非抛异常。
        baseline.compose("ab", newCursorPosition = 99)
        assertEquals(2, baseline.cursor)
    }

    @Test
    fun `textBefore and textAfter split the composing text at the cursor`() {
        val baseline = ComposingBaseline()
        // 绝对位置 0 落在组字开头：before 为空，after 从光标起取 length 个。
        baseline.compose("abcdef", newCursorPosition = 0)
        assertEquals("", baseline.textBefore(3))
        assertEquals("abc", baseline.textAfter(3))
        // 越界取文只给已存在的部分。
        assertEquals("abcdef", baseline.textAfter(99))
    }

    @Test
    fun `non-positive lengths return empty`() {
        val baseline = ComposingBaseline()
        baseline.compose("abc", newCursorPosition = 0)
        assertEquals("", baseline.textBefore(0))
        assertEquals("", baseline.textAfter(-1))
    }

    @Test
    fun `getters reflect only the composing baseline, never screen text`() {
        val baseline = ComposingBaseline()
        // 未组字时取文必须为空：输入法不得把终端屏幕内容当成自己的候选区。
        assertTrue(baseline.textBefore(10).isEmpty())
        assertTrue(baseline.textAfter(10).isEmpty())
        baseline.compose("ni", newCursorPosition = 0)
        assertEquals("", baseline.textBefore(5))
        assertEquals("ni", baseline.textAfter(5))
    }

    @Test
    fun `dropTrailingCodePoints removes whole code points`() {
        val baseline = ComposingBaseline()
        // 「a」+ 一个 emoji（代理对，UTF-16 长 2）= 3 个 UTF-16 单位、2 个码点。
        val emoji = "😀"
        baseline.compose("a$emoji", newCursorPosition = 0)
        // 按码点删 1 个：删掉的是整个 emoji（2 个 UTF-16 单位）。
        assertEquals(1, baseline.dropTrailingCodePoints(1))
        assertEquals("a", baseline.text)
    }

    @Test
    fun `dropTrailingCodePoints caps at the available length`() {
        val baseline = ComposingBaseline()
        baseline.compose("ab", newCursorPosition = 0)
        // 请求删 5 个，只有 2 个可删：返回实际删除数而非抛异常。
        assertEquals(2, baseline.dropTrailingCodePoints(5))
        assertEquals("", baseline.text)
        assertFalse(baseline.isComposing)
    }

    @Test
    fun `dropTrailingCodePoints pulls the cursor back`() {
        val baseline = ComposingBaseline()
        // 正值相对末尾偏移 1：光标落在末尾（6+1 钳到 6）。
        baseline.compose("abcdef", newCursorPosition = 1)
        assertEquals(6, baseline.cursor)
        assertEquals(2, baseline.dropTrailingCodePoints(2))
        // 删后文本短于光标，光标必须回落到新末尾，否则 textBefore 会越界。
        assertEquals(4, baseline.cursor)
        assertEquals("abcd", baseline.text)
        assertEquals("d", baseline.textBefore(1))
    }

    @Test
    fun `replaceRange splices head replacement and tail`() {
        val baseline = ComposingBaseline()
        baseline.compose("abcdef", newCursorPosition = 0)
        // 替换 [2,4)="cd" 为 "X"：头 "ab" + "X" + 尾 "ef"。
        assertEquals("abXef", baseline.replaceRange(2, 4, "X"))
    }

    @Test
    fun `replaceRange accepts an empty range as pure insertion`() {
        val baseline = ComposingBaseline()
        baseline.compose("ab", newCursorPosition = 0)
        assertEquals("aXYb", baseline.replaceRange(1, 1, "XY"))
    }

    @Test
    fun `replaceRange supports non-ascii replacement`() {
        val baseline = ComposingBaseline()
        baseline.compose("ni", newCursorPosition = 0)
        assertEquals("你", baseline.replaceRange(0, 2, "你"))
    }

    @Test
    fun `replaceRange rejects a range outside the baseline`() {
        val baseline = ComposingBaseline()
        baseline.compose("ab", newCursorPosition = 0)
        // 越界下标、倒置区间与负起点一律返回 null，调用方据此拒绝且不发字节。
        assertNull(baseline.replaceRange(0, 3, "X"))
        assertNull(baseline.replaceRange(3, 3, "X"))
        assertNull(baseline.replaceRange(-1, 1, "X"))
        assertNull(baseline.replaceRange(2, 1, "X"))
    }

    @Test
    fun `reanchorRegion returns only the addressed slice`() {
        val baseline = ComposingBaseline()
        baseline.compose("abcdef", newCursorPosition = 0)
        assertEquals("cd", baseline.reanchorRegion(2, 4))
        assertEquals("", baseline.reanchorRegion(3, 3))
    }

    @Test
    fun `reanchorRegion rejects a range outside the baseline`() {
        val baseline = ComposingBaseline()
        baseline.compose("ab", newCursorPosition = 0)
        assertNull(baseline.reanchorRegion(0, 9))
        assertNull(baseline.reanchorRegion(5, 5))
        assertNull(baseline.reanchorRegion(-2, 2))
    }

    @Test
    fun `reanchorRegion on an empty baseline rejects any non-empty range`() {
        val baseline = ComposingBaseline()
        // 未组字时无任何可重锚的基线：非空范围必须拒绝，
        // 否则会误删已提交内容（delta 的「无公共前缀即拒绝」语义）。
        assertNull(baseline.reanchorRegion(0, 1))
        assertEquals("", baseline.reanchorRegion(0, 0))
    }

    @Test
    fun `reconciling a replaceRange result rewrites only what follows the common prefix`() {
        val baseline = ComposingBaseline()
        baseline.compose("abcdef", newCursorPosition = 0)
        val target = checkNotNull(baseline.replaceRange(2, 4, "X")) { "replaceRange 必须成功" }
        // 与既有差分出口对接：`ComposingDiff` 的既定策略是「最长公共前缀 + 其后重打」，
        // 故前缀 "ab" 之后的 "cdef" 整体退格，再补新的 "Xef"。
        val edit = ComposingDiff.reconcile(baseline.text, target)
        assertEquals(4, edit.backspaces)
        assertEquals("Xef", edit.append)
    }
}
