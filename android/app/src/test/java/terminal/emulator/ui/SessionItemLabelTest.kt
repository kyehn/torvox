package terminal.emulator.ui

import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4

/**
 * 会话抽屉条目文案：`[序号] 标题`，序号加粗、标题斜体（termux 无会话名时的渲染）。
 *
 * 序号与标题必须同行：分两行渲染时，条目主文案只剩占位序号，终端标题被降级成
 * 第二行小字，与 DESIGN 的 `[1] ~` 示例不符。
 */
@RunWith(JUnit4::class)
class SessionItemLabelTest {
    @Test
    fun numberAndTitleShareOneLine() {
        val label = sessionItemLabel("[1]", "~")

        assertEquals("[1] ~", label.text)
        val bold = label.spanStyles.single { it.item.fontWeight == FontWeight.Bold }
        assertEquals(0, bold.start)
        assertEquals("[1] ".length, bold.end)
    }

    @Test
    fun titleIsItalic() {
        val label = sessionItemLabel("[2]", "vim README.md")

        val italic = label.spanStyles.single { it.item.fontStyle == FontStyle.Italic }
        assertEquals("[2] ".length, italic.start)
        assertEquals(label.text.length, italic.end)
        assertNull(italic.item.fontWeight)
    }

    @Test
    fun emptyTitleLeavesNumberOnly() {
        val label = sessionItemLabel("[3]", "")

        assertEquals("[3] ", label.text)
        assertEquals(1, label.spanStyles.size)
    }
}
