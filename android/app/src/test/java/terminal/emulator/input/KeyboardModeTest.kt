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
    fun `custom apply builds visible-password input type`() {
        val attrs = android.text.InputType.TYPE_NULL.let {
            android.view.inputmethod.EditorInfo().apply {
                inputType = it
                imeOptions = 0
            }
        }
        KeyboardMode.Custom(ImeFlagSet()).toEditorInfo(attrs)
        // VISIBLE_PASSWORD (0x91) must be present so CJK/voice composition
        // keeps working (haven docs: terminal hosts composition on top of a
        // VISIBLE_PASSWORD connection).
        assertTrue(attrs.inputType and android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD != 0)
    }
}
