package terminal.emulator.input

import android.text.InputType
import android.view.inputmethod.EditorInfo

sealed interface KeyboardMode {
    data object Secure : KeyboardMode

    data object Raw : KeyboardMode
}

fun KeyboardMode.toEditorInfo(outAttrs: EditorInfo) {
    when (this) {
        KeyboardMode.Secure -> {
            // 不受限的纯文本（termux 式）：NO_SUGGESTIONS 只把候选栏从终端屏上移除
            // ——它不限制输入法组字。刻意不使用 VISIBLE_PASSWORD
            // 与隐私输入法选项（NO_EXTRACT_UI / NO_PERSONALIZED_LEARNING）：
            // 那些会告知输入法这是密码/私密字段，Gboard 等因此
            // 完全丢弃组字与语言 UI——设备上表现为「非全屏模式」
            // （无法输入中文，且学习与剪贴板建议被禁用）。
            outAttrs.inputType =
                InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            outAttrs.imeOptions = EditorInfo.IME_ACTION_NONE
        }

        KeyboardMode.Raw -> {
            outAttrs.inputType = InputType.TYPE_NULL
            outAttrs.imeOptions =
                EditorInfo.IME_FLAG_NO_ENTER_ACTION or EditorInfo.IME_ACTION_NONE
        }
    }
}
