package terminal.emulator.input

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
    // VISIBLE_PASSWORD by default OFF: the password variation makes IMEs
    // (Gboard etc.) drop the composition/language UI, so CJK input stops
    // working. Opt-in for users who really want a password-style field.
    val visiblePassword: Boolean = false,
    val autoCorrect: Boolean = false,
    val fullEditor: Boolean = false,
    // No IME restrictions by default: NO_EXTRACT_UI / NO_PERSONALIZED_LEARNING
    // are opt-in. Privacy-style options make IMEs disable learning,
    // clipboard suggestions and sometimes the language switcher — the hard
    // requirement ("输入法不应该有任何限制") is an unrestricted IME.
    val noExtractUi: Boolean = false,
    val noPersonalizedLearning: Boolean = false,
)

fun KeyboardMode.toEditorInfo(outAttrs: EditorInfo) {
    when (this) {
        KeyboardMode.Secure -> {
            // Unrestricted plain text (termux-style): NO_SUGGESTIONS keeps
            // the suggestion strip off the terminal screen only — it does
            // not restrict IME composition. No VISIBLE_PASSWORD and no
            // privacy IME options (NO_EXTRACT_UI / NO_PERSONALIZED_LEARNING):
            // those tell the IME it is a password/private field, and Gboard
            // et al. respond by dropping the composition and language UI
            // entirely — reported on-device as "not full mode" where
            // Chinese cannot be typed, plus disabled learning and clipboard
            // suggestions. The IME is deliberately unrestricted.
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

    // Round-trip with toSettingsString(): a persisted "custom" must resolve
    // back to a Custom mode instead of falling into the Raw default (Haven
    // research: KeyboardMode 状态机往返一致性). Flags are not serialized —
    // the persisted value is just the mode selector; ImeFlagSet defaults are
    // applied on re-entry.
    "custom" -> KeyboardMode.Custom(ImeFlagSet())

    else -> KeyboardMode.Raw
}
