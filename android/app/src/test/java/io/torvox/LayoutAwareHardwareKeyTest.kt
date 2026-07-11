package io.torvox

import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import io.torvox.bridge.TorvoxBridge
import io.torvox.runtime.TorvoxRuntime
import io.torvox.ui.ModifierState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * I2 — Layout-aware hardware key handling.
 *
 * `TerminalViewModel.handleLayoutAwareHardwareKey` must use
 * `KeyEvent.getUnicodeChar(metaState)` so the produced character follows the
 * device's physical layout (e.g. Shift+2 -> '"' on German QWERTZ) instead of a
 * hardcoded US mapping, and must NOT intercept soft-keyboard / ACTION_UP /
 * modifier-only / Ctrl+key events (those go to the key-code encoder path).
 */
class LayoutAwareHardwareKeyTest {
    private fun buildViewModel(bridge: TorvoxBridge?): TerminalViewModel {
        val context = mockk<android.content.Context>(relaxed = true)
        val repository = mockk<io.torvox.settings.SettingsRepository>(relaxed = true)
        val runtime = mockk<TorvoxRuntime>(relaxed = true)
        every { runtime.bridge() } returns bridge
        return TerminalViewModel(context, repository, runtime)
    }

    private fun physicalKeyEvent(
        keyCode: Int,
        metaState: Int,
        action: Int = KeyEvent.ACTION_DOWN,
        unicodeChar: Int,
        softKeyboard: Boolean = false,
        virtualDevice: Boolean = false,
    ): KeyEvent {
        val event = mockk<KeyEvent>(relaxed = true)
        every { event.action } returns action
        every { event.keyCode } returns keyCode
        every { event.metaState } returns metaState
        every { event.getUnicodeChar(any()) } returns unicodeChar
        every { event.isFromSource(InputDevice.SOURCE_KEYBOARD) } returns true
        every { event.deviceId } returns if (virtualDevice) KeyCharacterMap.VIRTUAL_KEYBOARD else 0
        every { event.flags } returns if (softKeyboard) KeyEvent.FLAG_SOFT_KEYBOARD else 0
        return event
    }

    @Test
    fun germanQwertzShiftTwoProducesQuoteChar() {
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        every { bridge.processKeyEvent(KeyEvent.KEYCODE_2, 0, 0, '"'.code, '"'.code) } returns true
        val viewModel = buildViewModel(bridge)

        // Shift+2 on a German QWERTZ layout yields '"' (0x22), not '@'.
        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_2,
                metaState = KeyEvent.META_SHIFT_ON,
                unicodeChar = '"'.code,
            )

        assertTrue(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 1) {
            bridge.processKeyEvent(KeyEvent.KEYCODE_2, 0, 0, '"'.code, '"'.code)
        }
    }

    @Test
    fun germanQwertzAltGrQProducesAtSign() {
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        every {
            bridge.processKeyEvent(KeyEvent.KEYCODE_Q, 0, 0, '@'.code, '@'.code)
        } returns true
        val viewModel = buildViewModel(bridge)

        // AltGr+Q on German QWERTZ yields '@' (0x40).
        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_Q,
                metaState = KeyEvent.META_ALT_RIGHT_ON,
                unicodeChar = '@'.code,
            )

        assertTrue(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 1) {
            bridge.processKeyEvent(KeyEvent.KEYCODE_Q, 0, 0, '@'.code, '@'.code)
        }
    }

    @Test
    fun ignoresSoftKeyboardEvents() {
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        val viewModel = buildViewModel(bridge)
        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_A,
                metaState = 0,
                unicodeChar = 'a'.code,
                softKeyboard = true,
            )
        assertFalse(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 0) { bridge.processKeyEvent(any(), any(), any(), any(), any()) }
    }

    @Test
    fun ignoresActionUp() {
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        val viewModel = buildViewModel(bridge)
        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_A,
                metaState = 0,
                action = KeyEvent.ACTION_UP,
                unicodeChar = 'a'.code,
            )
        assertFalse(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 0) { bridge.processKeyEvent(any(), any(), any(), any(), any()) }
    }

    @Test
    fun ignoresModifierOnlyKeys() {
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        val viewModel = buildViewModel(bridge)
        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_SHIFT_LEFT,
                metaState = 0,
                unicodeChar = 0,
            )
        assertFalse(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 0) { bridge.processKeyEvent(any(), any(), any(), any(), any()) }
    }

    @Test
    fun ignoresCtrlComboWithoutAltGr() {
        // Ctrl+key is key-code based and handled by the Ghostty encoder, not here.
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        val viewModel = buildViewModel(bridge)
        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_A,
                metaState = KeyEvent.META_CTRL_ON,
                unicodeChar = 1,
            )
        assertFalse(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 0) { bridge.processKeyEvent(any(), any(), any(), any(), any()) }
    }

    @Test
    fun forwardsStickyLockedCtrlAsMask() {
        // A Locked Ctrl sticky modifier must be forwarded as mask bit 4 (0x04)
        // while the character still comes from getUnicodeChar.
        val bridge = mockk<TorvoxBridge>(relaxed = true)
        every { bridge.processKeyEvent(KeyEvent.KEYCODE_A, 4, 0, 'a'.code, 'a'.code) } returns true
        val viewModel = buildViewModel(bridge)
        viewModel.cycleCtrlState() // Off -> Once
        viewModel.cycleCtrlState() // Once -> Locked

        val event =
            physicalKeyEvent(
                keyCode = KeyEvent.KEYCODE_A,
                metaState = 0,
                unicodeChar = 'a'.code,
            )
        assertTrue(viewModel.handleLayoutAwareHardwareKey(event))
        verify(exactly = 1) {
            bridge.processKeyEvent(KeyEvent.KEYCODE_A, 4, 0, 'a'.code, 'a'.code)
        }
        // Locked modifier must survive the keystroke (only Once is consumed).
        assertEquals(ModifierState.Locked, viewModel.state.value.ctrlState)
    }
}
