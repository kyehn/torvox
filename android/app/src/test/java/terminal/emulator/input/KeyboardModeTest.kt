package terminal.emulator.input

import org.junit.Assert.assertEquals
import org.junit.Test

/** KeyboardMode 无持久化入口（不提供切换 UI），只断言各模式到 EditorInfo 的映射不崩且可区分。 */
class KeyboardModeTest {

    @Test
    fun `every mode maps to an editor info without colliding`() {
        val seen = mutableSetOf<Int>()
        for (mode in
            listOf(
                KeyboardMode.Secure,
                KeyboardMode.Standard,
                KeyboardMode.Raw,
                KeyboardMode.Custom(ImeFlagSet()),
            )
        ) {
            val outAttrs = android.view.inputmethod.EditorInfo()
            mode.toEditorInfo(outAttrs)
            assertEquals("模式必须真的写入 imeOptions", true, outAttrs.imeOptions != 0)
            assertEquals("不同模式不得映射到同一组 flags", true, seen.add(outAttrs.imeOptions))
        }
    }
}
