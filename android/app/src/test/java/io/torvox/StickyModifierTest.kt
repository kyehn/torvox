package io.torvox

import io.mockk.mockk
import io.torvox.runtime.TorvoxRuntime
import io.torvox.ui.ModifierState
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * I5 — Sticky (one-shot "tapped") modifier clearing.
 *
 * A one-shot modifier (ModifierState.Once) is consumed after the keystroke so
 * it cannot persist into the next keystroke. `consumeOneShotModifiers()` clears
 * Once -> Off while leaving Locked untouched. The real ViewModel is exercised
 * (its `consumeOneShotModifiers` operates on `_state.value`, no native calls).
 */
class StickyModifierTest {
    private fun buildViewModel(): TerminalViewModel {
        val context = mockk<android.content.Context>(relaxed = true)
        val repository = mockk<io.torvox.settings.SettingsRepository>(relaxed = true)
        val runtime = mockk<TorvoxRuntime>(relaxed = true)
        return TerminalViewModel(context, repository, runtime)
    }

    @Test
    fun consumeOneShot_clearsCtrlOnce() {
        val viewModel = buildViewModel()
        viewModel.cycleCtrlState() // Off -> Once
        assertEquals(ModifierState.Once, viewModel.state.value.ctrlState)

        viewModel.consumeOneShotModifiers()
        assertEquals(ModifierState.Off, viewModel.state.value.ctrlState)
    }

    @Test
    fun consumeOneShot_clearsAltOnce() {
        val viewModel = buildViewModel()
        viewModel.cycleAltState() // Off -> Once
        assertEquals(ModifierState.Once, viewModel.state.value.altState)

        viewModel.consumeOneShotModifiers()
        assertEquals(ModifierState.Off, viewModel.state.value.altState)
    }

    @Test
    fun consumeOneShot_clearsBothOnce() {
        val viewModel = buildViewModel()
        viewModel.cycleCtrlState()
        viewModel.cycleAltState()
        assertEquals(ModifierState.Once, viewModel.state.value.ctrlState)
        assertEquals(ModifierState.Once, viewModel.state.value.altState)

        viewModel.consumeOneShotModifiers()
        assertEquals(ModifierState.Off, viewModel.state.value.ctrlState)
        assertEquals(ModifierState.Off, viewModel.state.value.altState)
    }

    @Test
    fun consumeOneShot_preservesLocked() {
        val viewModel = buildViewModel()
        viewModel.cycleCtrlState() // Once
        viewModel.cycleCtrlState() // Locked
        viewModel.cycleAltState()
        viewModel.cycleAltState() // Locked
        assertEquals(ModifierState.Locked, viewModel.state.value.ctrlState)
        assertEquals(ModifierState.Locked, viewModel.state.value.altState)

        viewModel.consumeOneShotModifiers()
        // Locked must survive consumption; only Once would be cleared.
        assertEquals(ModifierState.Locked, viewModel.state.value.ctrlState)
        assertEquals(ModifierState.Locked, viewModel.state.value.altState)
    }
}
