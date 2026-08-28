package terminal.emulator.input

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class KeyboardModeTest {
    @Test
    fun `toKeyboardMode round-trips every persisted mode string`() {
        // toSettingsString() is the inverse of toKeyboardMode() for all four
        // modes; a persisted value must never fall through to the Raw default
        // (haven research: KeyboardMode state-machine round-trip consistency).
        for (mode in listOf(
            KeyboardMode.Secure,
            KeyboardMode.Standard,
            KeyboardMode.Raw,
            KeyboardMode.Custom(ImeFlagSet()),
        )) {
            assertEquals(
                "toSettingsString/toKeyboardMode round-trip failed for $mode",
                mode,
                mode.toSettingsString().toKeyboardMode(),
            )
        }
    }

    @Test
    fun `custom mode resolves to Custom with default flags`() {
        val mode = "custom".toKeyboardMode()
        assertTrue("expected Custom, got $mode", mode is KeyboardMode.Custom)
        // Deserialized Custom re-applies the ImeFlagSet defaults.
        val custom = mode as KeyboardMode.Custom
        assertEquals(ImeFlagSet(), custom.flags)
    }

    @Test
    fun `unknown persisted value falls back to Raw`() {
        assertEquals(KeyboardMode.Raw, "bogus-mode".toKeyboardMode())
    }

    @Test
    fun `custom apply default flags keep full-ime composition`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Custom(ImeFlagSet()).toEditorInfo(attrs)
        // Default flags must NOT include VISIBLE_PASSWORD: the password
        // variation makes IMEs (Gboard et al.) drop the composition and
        // language UI so CJK/voice input stops working (on-device report).
        assertEquals(
            0,
            attrs.inputType and android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
        )
        assertTrue(attrs.inputType and android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS != 0)
    }

    @Test
    fun `custom visiblePassword flag opts back into password field`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Custom(ImeFlagSet(visiblePassword = true)).toEditorInfo(attrs)
        assertTrue(attrs.inputType and android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD != 0)
    }

    @Test
    fun `standard mode allows autocorrect and hides action bar`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Standard.toEditorInfo(attrs)
        assertTrue(attrs.inputType and android.text.InputType.TYPE_TEXT_FLAG_AUTO_CORRECT != 0)
        assertTrue(attrs.imeOptions and android.view.inputmethod.EditorInfo.IME_FLAG_NO_ENTER_ACTION != 0)
    }

    @Test
    fun `secure mode is unrestricted plain text without password variation`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Secure.toEditorInfo(attrs)
        // No VISIBLE_PASSWORD and no privacy IME options: the password
        // variation and IME_FLAG_NO_* options make IMEs (Gboard etc.) drop
        // the composition/language UI, learning and clipboard features —
        // the hard requirement is an IME with no restrictions. Only
        // NO_SUGGESTIONS remains, keeping the strip off the terminal
        // screen without restricting composition.
        assertEquals(
            0,
            attrs.inputType and android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
        )
        assertTrue(attrs.inputType and android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS != 0)
        assertEquals(0, attrs.imeOptions and android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI)
        assertEquals(0, attrs.imeOptions and android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING)
    }

    @Test
    fun `custom default flags carry no ime restrictions`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Custom(ImeFlagSet()).toEditorInfo(attrs)
        assertEquals(0, attrs.imeOptions and android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI)
        assertEquals(0, attrs.imeOptions and android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING)
    }

    @Test
    fun `raw mode uses null input type`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Raw.toEditorInfo(attrs)
        assertEquals(android.text.InputType.TYPE_NULL, attrs.inputType)
    }

    @Test
    fun `custom flags toggle autocorrect and full editor`() {
        val attrs = freshEditorInfo()
        KeyboardMode.Custom(
            ImeFlagSet(
                noSuggestions = false,
                autoCorrect = true,
                fullEditor = true,
            ),
        ).toEditorInfo(attrs)
        assertTrue(attrs.inputType and android.text.InputType.TYPE_TEXT_FLAG_AUTO_CORRECT != 0)
        // fullEditor clears NO_EXTRACT_UI (editor may take the full screen).
        assertEquals(0, attrs.imeOptions and android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI)
    }

    private fun freshEditorInfo() = android.view.inputmethod.EditorInfo().apply {
        inputType = android.text.InputType.TYPE_NULL
        imeOptions = 0
    }
}
