package terminal.emulator.input

import android.text.InputType
import android.view.inputmethod.EditorInfo
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 终端输入法编辑器属性的契约（DESIGN 要求全功能输入法，无切换入口，故只有一种形态）：
 * 不得带密码变体与隐私 IME 选项，否则 Gboard 等会丢弃组字与语言 UI。
 */
class TerminalEditorInfoTest {

    private fun editorInfo(): EditorInfo = EditorInfo().also(::applyTerminalEditorInfo)

    @Test
    fun `编辑器属性为不受限多行纯文本`() {
        val outAttrs = editorInfo()
        assertEquals(
            InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS,
            outAttrs.inputType,
        )
        // 回车键形态由 MULTI_LINE 决定（否则触发 performEditorAction，终端无动作可执行），
        // 动作位一律留 UNSPECIFIED；IME_ACTION_NONE 会让屏幕键盘无法输入换行（termux-app#221）。
        assertEquals(
            EditorInfo.IME_FLAG_NO_FULLSCREEN,
            outAttrs.imeOptions and EditorInfo.IME_FLAG_NO_FULLSCREEN,
        )
        assertEquals(
            EditorInfo.IME_ACTION_UNSPECIFIED,
            outAttrs.imeOptions and EditorInfo.IME_MASK_ACTION,
        )
    }

    @Test
    fun `不带密码变体与隐私输入法标志`() {
        val outAttrs = editorInfo()
        for (
        variation in
        listOf(
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        )
        ) {
            assertEquals(
                "不得带密码变体 $variation",
                0,
                outAttrs.inputType and (variation shr 8),
            )
        }
        for (
        flag in
        listOf(EditorInfo.IME_FLAG_NO_EXTRACT_UI, EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING)
        ) {
            assertEquals("不得限制输入法 $flag", 0, outAttrs.imeOptions and flag)
        }
        assertEquals(
            "必须是富文本编辑器（TYPE_CLASS_TEXT），否则 CJK 组合输入不可用",
            InputType.TYPE_CLASS_TEXT,
            outAttrs.inputType and InputType.TYPE_MASK_CLASS,
        )
    }
}
