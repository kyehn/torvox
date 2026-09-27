package terminal.emulator.input

import android.text.InputType
import android.view.inputmethod.EditorInfo

sealed interface KeyboardMode {
    data object Secure : KeyboardMode

    data object Standard : KeyboardMode

    data object Raw : KeyboardMode

    data class Custom(val flags: ImeFlagSet) : KeyboardMode
}

data class ImeFlagSet(
    val noSuggestions: Boolean = true,
    // VISIBLE_PASSWORD 默认关闭：密码变体会让输入法（Gboard 等）
    // 丢弃组字/语言 UI，导致 CJK 输入失效。
    // 仅供确实需要密码式字段的用户主动开启。
    val visiblePassword: Boolean = false,
    val autoCorrect: Boolean = false,
    val fullEditor: Boolean = false,
    // 默认不对输入法施加任何限制：NO_EXTRACT_UI / NO_PERSONALIZED_LEARNING
    // 均为选择性开启。隐私类选项会让输入法禁用学习、剪贴板建议，
    // 有时连语言切换器也一并禁用——而硬性要求（「输入法不应该有任何限制」）
    // 是不受限的输入法。
    val noExtractUi: Boolean = false,
    val noPersonalizedLearning: Boolean = false,
)

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

        KeyboardMode.Standard -> {
            outAttrs.inputType =
                InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
            outAttrs.imeOptions =
                EditorInfo.IME_FLAG_NO_ENTER_ACTION or EditorInfo.IME_ACTION_NONE
        }

        KeyboardMode.Raw -> {
            outAttrs.inputType = InputType.TYPE_NULL
            outAttrs.imeOptions =
                EditorInfo.IME_FLAG_NO_ENTER_ACTION or EditorInfo.IME_ACTION_NONE
        }

        is KeyboardMode.Custom -> {
            var inputType = InputType.TYPE_CLASS_TEXT
            if (flags.noSuggestions) {
                inputType = inputType or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            }
            if (flags.autoCorrect) {
                inputType = inputType or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
            }
            if (flags.visiblePassword) {
                inputType = inputType or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            }
            outAttrs.inputType = inputType

            var imeOptions = EditorInfo.IME_FLAG_NO_ENTER_ACTION or EditorInfo.IME_ACTION_NONE
            if (flags.noExtractUi) {
                imeOptions = imeOptions or EditorInfo.IME_FLAG_NO_EXTRACT_UI
            }
            if (flags.noPersonalizedLearning) {
                imeOptions = imeOptions or EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
            }
            if (flags.fullEditor) {
                imeOptions = imeOptions and EditorInfo.IME_FLAG_NO_EXTRACT_UI.inv()
            }
            outAttrs.imeOptions = imeOptions
        }
    }
}
