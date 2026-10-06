package terminal.emulator.input

import android.text.InputType
import android.view.inputmethod.EditorInfo

/**
 * 终端的输入法编辑器属性：DESIGN 要求全功能输入法，不提供切换入口，故只有一种形态。
 *
 * 不受限的纯文本（termux 式）：NO_SUGGESTIONS 只把候选栏从终端屏上移除——它不限制
 * 输入法组字。刻意不使用 VISIBLE_PASSWORD 与隐私输入法选项
 * （NO_EXTRACT_UI / NO_PERSONALIZED_LEARNING）：那些会告知输入法这是密码/私密字段，
 * Gboard 等因此完全丢弃组字与语言 UI——设备上表现为「非全屏模式」（无法输入中文，
 * 且学习与剪贴板建议被禁用）。
 */
fun applyTerminalEditorInfo(outAttrs: EditorInfo) {
    outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
    outAttrs.imeOptions = EditorInfo.IME_ACTION_NONE
}
