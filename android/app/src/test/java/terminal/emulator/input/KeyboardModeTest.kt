package terminal.emulator.input

import android.text.InputType
import android.view.inputmethod.EditorInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * KeyboardMode 无持久化入口（不提供切换 UI），因此只断言 toEditorInfo 的实际契约。
 * Secure 是唯一被使用的模式，其契约是「不限制输入法特性」：不得带密码变体与隐私 IME 选项。
 */
class KeyboardModeTest {

    private fun editorInfoOf(mode: KeyboardMode): EditorInfo {
        val outAttrs = EditorInfo()
        mode.toEditorInfo(outAttrs)
        return outAttrs
    }

    @Test
    fun `secure mode is unrestricted text`() {
        val outAttrs = editorInfoOf(KeyboardMode.Secure)
        assertEquals(
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS,
            outAttrs.inputType,
        )
        assertEquals(EditorInfo.IME_ACTION_NONE, outAttrs.imeOptions)
    }

    @Test
    fun `secure mode carries no password variation nor privacy ime flags`() {
        val outAttrs = editorInfoOf(KeyboardMode.Secure)
        for (
        variation in
        listOf(
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        )
        ) {
            assertEquals(
                "Secure 模式不得带密码变体 $variation",
                0,
                outAttrs.inputType and (variation shr 8),
            )
        }
        for (
        flag in
        listOf(EditorInfo.IME_FLAG_NO_EXTRACT_UI, EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING)
        ) {
            assertEquals("Secure 模式不得限制输入法 $flag", 0, outAttrs.imeOptions and flag)
        }
        assertEquals(
            "Secure 模式必须是富文本编辑器（TYPE_CLASS_TEXT），否则 CJK 组合输入不可用",
            InputType.TYPE_CLASS_TEXT,
            outAttrs.inputType and InputType.TYPE_MASK_CLASS,
        )
    }

    @Test
    fun `custom mode applies only the requested privacy flags`() {
        val plain = editorInfoOf(KeyboardMode.Custom(ImeFlagSet()))
        val restricted = editorInfoOf(KeyboardMode.Custom(ImeFlagSet(noExtractUi = true)))
        assertEquals(0, plain.imeOptions and EditorInfo.IME_FLAG_NO_EXTRACT_UI)
        assertTrue(restricted.imeOptions and EditorInfo.IME_FLAG_NO_EXTRACT_UI != 0)
    }

    @Test
    fun `raw mode uses TYPE_NULL`() {
        assertEquals(InputType.TYPE_NULL, editorInfoOf(KeyboardMode.Raw).inputType)
    }
}
