package io.torvox.ui

import android.text.InputType
import android.view.inputmethod.EditorInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * I1 — KeyboardMode.Secure preserves IME composition.
 *
 * `Secure` must use a real text input type
 * (TYPE_CLASS_TEXT | TYPE_TEXT_VARIATION_VISIBLE_PASSWORD |
 * TYPE_TEXT_FLAG_NO_SUGGESTIONS) rather than `TYPE_NULL`, which kills CJK /
 * voice / swipe composition.
 */
class KeyboardModeTest {
    @Test
    fun secure_usesVisiblePasswordTextInputType() {
        val editorInfo = EditorInfo()
        KeyboardMode.Secure.toEditorInfo(editorInfo)
        val expected =
            InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        assertEquals(expected, editorInfo.inputType)
    }

    @Test
    fun secure_isNotTypeNull() {
        val editorInfo = EditorInfo()
        KeyboardMode.Secure.toEditorInfo(editorInfo)
        assertNotEquals(InputType.TYPE_NULL, editorInfo.inputType)
    }

    @Test
    fun standard_usesClassTextWithAutoCorrect() {
        val editorInfo = EditorInfo()
        KeyboardMode.Standard.toEditorInfo(editorInfo)
        assertEquals(
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT,
            editorInfo.inputType,
        )
    }

    @Test
    fun raw_usesTypeNull() {
        val editorInfo = EditorInfo()
        KeyboardMode.Raw.toEditorInfo(editorInfo)
        assertEquals(InputType.TYPE_NULL, editorInfo.inputType)
    }

    @Test
    fun custom_visiblePasswordNoSuggestions() {
        val editorInfo = EditorInfo()
        KeyboardMode.Custom(ImeFlagSet()).toEditorInfo(editorInfo)
        val expected =
            InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        assertEquals(expected, editorInfo.inputType)
    }
}
