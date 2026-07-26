package io.term.ui

import android.text.InputType
import android.view.inputmethod.EditorInfo

sealed interface KeyboardMode {
    data object Secure : KeyboardMode

    data object Standard : KeyboardMode

    data object Raw : KeyboardMode

    data class Custom(
        val flags: ImeFlagSet,
    ) : KeyboardMode
}

data class ImeFlagSet(
    val noSuggestions: Boolean = true,
    val visiblePassword: Boolean = true,
    val autoCorrect: Boolean = false,
    val fullEditor: Boolean = false,
    val noExtractUi: Boolean = true,
    val noPersonalizedLearning: Boolean = true,
)

fun KeyboardMode.toEditorInfo(outAttrs: EditorInfo) {
    when (this) {
        KeyboardMode.Secure -> {
            // VISIBLE_PASSWORD | NO_SUGGESTIONS keeps the privacy lock (no
            // suggestion strip, no personalized learning, no autocorrect) while
            // still allowing the IME to host its own composition flow, so CJK /
            // voice / swipe input works. TYPE_NULL would kill composition
            // entirely (Haven docs: CJK still works because the terminal hosts
            // composition on top of a VISIBLE_PASSWORD connection).
            outAttrs.inputType =
                InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            outAttrs.imeOptions =
                EditorInfo.IME_FLAG_NO_EXTRACT_UI or
                EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING
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

fun KeyboardMode.toSettingsString(): String = when (this) {
    KeyboardMode.Secure -> "secure"
    KeyboardMode.Standard -> "standard"
    KeyboardMode.Raw -> "raw"
    is KeyboardMode.Custom -> "custom"
}

fun String.toKeyboardMode(): KeyboardMode = when (this) {
    "secure" -> KeyboardMode.Secure
    "standard" -> KeyboardMode.Standard
    "raw" -> KeyboardMode.Raw
    else -> KeyboardMode.Raw
}
