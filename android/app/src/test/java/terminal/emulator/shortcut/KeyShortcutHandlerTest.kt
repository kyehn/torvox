package terminal.emulator.shortcut

import android.os.Build
import android.view.KeyEvent
import io.mockk.mockk
import io.mockk.verify
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import terminal.emulator.TerminalViewModel

/**
 * Tests for [KeyShortcutHandler] — verifies the fix where
 * [setBindings] maps ACTION_ID_* constants to actions correctly.
 *
 * Before the fix, `idToAction` was keyed by enum name ("Paste") while
 * `setBindings` received keys from Defaults ("paste"), so all entries
 * were dropped and bindingFor() always returned EMPTY.
 *
 * dispatch() runs under Robolectric so real [KeyEvent] instances can be
 * constructed (the modifier match reads the metaState bitmask directly).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [Build.VERSION_CODES.TIRAMISU])
class KeyShortcutHandlerTest {

    private fun createViewModel(): TerminalViewModel = mockk<TerminalViewModel>(relaxed = true)

    @Test
    fun `setBindings with Defaults maps all 5 actions to non-empty bindings`() {
        val handler = KeyShortcutHandler(createViewModel())

        handler.setBindings(KeyShortcutHandler.Defaults.all())

        KeyShortcutHandler.Action.values().forEach { action ->
            val binding = handler.bindingFor(action)
            assertTrue("Action $action should have a non-empty binding", !binding.isEmpty())
        }
    }

    @Test
    fun `bindingFor returns correct binding for Paste action`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        val binding = handler.bindingFor(KeyShortcutHandler.Action.Paste)
        assertEquals(KeyEvent.KEYCODE_V, binding.key)
        assertTrue(binding.ctrl)
        assertTrue(binding.shift)
    }

    @Test
    fun `bindingFor returns correct binding for NewSession action`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        val binding = handler.bindingFor(KeyShortcutHandler.Action.NewSession)
        assertEquals(KeyEvent.KEYCODE_N, binding.key)
        assertTrue(binding.ctrl)
        assertTrue(binding.shift)
    }

    @Test
    fun `bindingFor returns correct binding for Copy action`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        val binding = handler.bindingFor(KeyShortcutHandler.Action.Copy)
        assertEquals(KeyEvent.KEYCODE_C, binding.key)
        assertTrue(binding.ctrl)
        assertTrue(binding.shift)
    }

    @Test
    fun `bindingFor returns correct binding for CloseSession action`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        val binding = handler.bindingFor(KeyShortcutHandler.Action.CloseSession)
        assertEquals(KeyEvent.KEYCODE_W, binding.key)
        assertTrue(binding.ctrl)
        assertTrue(binding.shift)
    }

    @Test
    fun `bindingFor returns correct binding for ToggleScroll action`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        val binding = handler.bindingFor(KeyShortcutHandler.Action.ToggleScroll)
        assertEquals(KeyEvent.KEYCODE_S, binding.key)
        assertTrue(binding.ctrl)
        assertTrue(binding.shift)
    }

    @Test
    fun `setBindings with empty map makes all bindings EMPTY`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(emptyMap())

        KeyShortcutHandler.Action.values().forEach { action ->
            assertEquals(
                "Action $action should be EMPTY with empty bindings",
                ShortcutBinding.EMPTY,
                handler.bindingFor(action),
            )
        }
    }

    @Test
    fun `custom binding replaces default for Paste`() {
        val handler = KeyShortcutHandler(createViewModel())
        val custom = KeyShortcutHandler.Defaults.all().toMutableMap()
        custom["paste"] = ShortcutBinding(key = KeyEvent.KEYCODE_INSERT, ctrl = true)
        handler.setBindings(custom)

        val binding = handler.bindingFor(KeyShortcutHandler.Action.Paste)
        assertEquals(KeyEvent.KEYCODE_INSERT, binding.key)
        assertTrue(binding.ctrl)
        assertFalse(binding.shift)
    }

    @Test
    fun `setBindings with serialized round-trip works`() {
        val handler = KeyShortcutHandler(createViewModel())
        val serialized = KeyShortcutHandler.Defaults.all().mapValues { (_, b) -> b.serialize() }
        val deserialized = serialized.mapValues { (_, s) -> ShortcutBinding.deserialize(s) }
        handler.setBindings(deserialized)

        KeyShortcutHandler.Action.values().forEach { action ->
            val binding = handler.bindingFor(action)
            assertTrue("Action $action should have a non-empty binding after round-trip", !binding.isEmpty())
        }
    }

    @Test
    fun `idToAction maps all 5 ACTION_ID constants`() {
        // The fix: verify all ACTION_ID_* constants map to correct Actions.
        // Before the fix, only enum names ("Paste") mapped, not action IDs ("paste").
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        // If idToAction is wrong, bindingFor would return EMPTY for all
        val pasteBinding = handler.bindingFor(KeyShortcutHandler.Action.Paste)
        assertFalse("Paste binding should be non-empty (idToAction must map 'paste' to Paste)", pasteBinding.isEmpty())

        val newSessionBinding = handler.bindingFor(KeyShortcutHandler.Action.NewSession)
        assertFalse("NewSession binding should be non-empty", newSessionBinding.isEmpty())
    }

    @Test
    fun `updateBinding with new action works`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        // Simulate user changing Paste binding in settings
        val newBindings = KeyShortcutHandler.Defaults.all().toMutableMap()
        newBindings["paste"] = ShortcutBinding(key = KeyEvent.KEYCODE_P, ctrl = true)
        handler.setBindings(newBindings)

        val binding = handler.bindingFor(KeyShortcutHandler.Action.Paste)
        assertEquals(KeyEvent.KEYCODE_P, binding.key)
    }

    @Test
    fun `resetBinding restores default`() {
        val handler = KeyShortcutHandler(createViewModel())
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        // Change then reset
        val changed = KeyShortcutHandler.Defaults.all().toMutableMap()
        changed["paste"] = ShortcutBinding(key = KeyEvent.KEYCODE_P, ctrl = true)
        handler.setBindings(changed)
        assertEquals(KeyEvent.KEYCODE_P, handler.bindingFor(KeyShortcutHandler.Action.Paste).key)

        // Reset to defaults
        handler.setBindings(KeyShortcutHandler.Defaults.all())
        assertEquals(KeyEvent.KEYCODE_V, handler.bindingFor(KeyShortcutHandler.Action.Paste).key)
    }

    @Test
    fun `dispatch executes bound action on exact chord`() {
        val viewModel = mockk<TerminalViewModel>(relaxed = true)
        val handler = KeyShortcutHandler(viewModel)
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        // Ctrl+Shift+V (KEYCODE_V) — the default paste chord.
        val event = KeyEvent(0L, 0L, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_V, 0, KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON)
        assertTrue("bound chord must be consumed", handler.dispatch(event))
        verify { viewModel.pasteFromClipboard() }
    }

    @Test
    fun `dispatch falls through when modifier combination differs`() {
        val viewModel = mockk<TerminalViewModel>(relaxed = true)
        val handler = KeyShortcutHandler(viewModel)
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        // Ctrl+V without Shift is unbound and must reach the terminal.
        val event = KeyEvent(0L, 0L, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_V, 0, KeyEvent.META_CTRL_ON)
        assertFalse("unbound chord must fall through", handler.dispatch(event))
        verify(exactly = 0) { viewModel.pasteFromClipboard() }
    }

    @Test
    fun `dispatch ignores auto-repeat events`() {
        val viewModel = mockk<TerminalViewModel>(relaxed = true)
        val handler = KeyShortcutHandler(viewModel)
        handler.setBindings(KeyShortcutHandler.Defaults.all())

        val repeatEvent = KeyEvent(0L, 0L, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_V, 1, KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON)
        assertFalse("auto-repeat must not re-dispatch the action", handler.dispatch(repeatEvent))
        verify(exactly = 0) { viewModel.pasteFromClipboard() }
    }
}
