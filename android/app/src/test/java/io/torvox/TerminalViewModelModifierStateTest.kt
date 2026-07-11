package io.torvox

import io.torvox.ui.ModifierKey
import io.torvox.ui.ModifierState
import io.torvox.ui.defaultModifierKeys
import io.torvox.ui.next
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalViewModelModifierStateTest {
    @Test
    fun modifierStateCyclingFullSequence() {
        assertEquals(ModifierState.Once, ModifierState.Off.next())
        assertEquals(ModifierState.Locked, ModifierState.Once.next())
        assertEquals(ModifierState.Off, ModifierState.Locked.next())
        assertEquals(ModifierState.Once, ModifierState.Off.next())
    }

    @Test
    fun consumeOneShotModifiers_resetsOnlyOnce() {
        val delegate = ModifierStateDelegate(TerminalState(ctrlState = ModifierState.Once, altState = ModifierState.Locked))
        val result = delegate.consumeOneShotModifiers()
        assertEquals(ModifierState.Off, result.ctrlState)
        assertEquals(ModifierState.Locked, result.altState)
    }

    @Test
    fun consumeOneShotModifiers_preservesOff() {
        val delegate = ModifierStateDelegate(TerminalState(ctrlState = ModifierState.Off, altState = ModifierState.Off))
        val result = delegate.consumeOneShotModifiers()
        assertEquals(ModifierState.Off, result.ctrlState)
        assertEquals(ModifierState.Off, result.altState)
    }

    @Test
    fun consumeOneShotModifiers_resetsBothOnce() {
        val delegate = ModifierStateDelegate(TerminalState(ctrlState = ModifierState.Once, altState = ModifierState.Once))
        val result = delegate.consumeOneShotModifiers()
        assertEquals(ModifierState.Off, result.ctrlState)
        assertEquals(ModifierState.Off, result.altState)
    }

    @Test
    fun consumeOneShotModifiers_preservesLocked() {
        val delegate = ModifierStateDelegate(TerminalState(ctrlState = ModifierState.Locked, altState = ModifierState.Locked))
        val result = delegate.consumeOneShotModifiers()
        assertEquals(ModifierState.Locked, result.ctrlState)
        assertEquals(ModifierState.Locked, result.altState)
    }

    @Test
    fun cycleCtrlState_cyclesForward() {
        val delegate = ModifierStateDelegate(TerminalState(ctrlState = ModifierState.Off))
        val afterOnce = delegate.cycleCtrlState()
        assertEquals(ModifierState.Once, afterOnce.ctrlState)
        val afterLocked = delegate.cycleCtrlState(afterOnce)
        assertEquals(ModifierState.Locked, afterLocked.ctrlState)
        val afterOff = delegate.cycleCtrlState(afterLocked)
        assertEquals(ModifierState.Off, afterOff.ctrlState)
    }

    @Test
    fun cycleAltState_cyclesForward() {
        val delegate = ModifierStateDelegate(TerminalState(altState = ModifierState.Off))
        val afterOnce = delegate.cycleAltState()
        assertEquals(ModifierState.Once, afterOnce.altState)
        val afterLocked = delegate.cycleAltState(afterOnce)
        assertEquals(ModifierState.Locked, afterLocked.altState)
    }

    @Test
    fun consumePendingInput_returnsAndClears() {
        val input = byteArrayOf(0x48, 0x69)
        val delegate = ModifierStateDelegate(TerminalState(pendingInput = input))
        val result = delegate.consumePendingInput()
        assertArrayEquals(input, result.input)
        assertNull(result.state.pendingInput)
    }

    @Test
    fun consumePendingInput_whenNullReturnsNull() {
        val delegate = ModifierStateDelegate(TerminalState(pendingInput = null))
        val result = delegate.consumePendingInput()
        assertNull(result.input)
        assertNull(result.state.pendingInput)
    }

    @Test
    fun setModifierKeys_updatesKeys() {
        val keys = listOf(ModifierKey(key = "a", display = "A"), ModifierKey(key = "b", display = "B"))
        val delegate = ModifierStateDelegate(TerminalState())
        val result = delegate.setModifierKeys(keys)
        assertEquals(2, result.modifierKeys.size)
        assertEquals("a", result.modifierKeys[0].key)
        assertEquals("A", result.modifierKeys[0].label)
        assertEquals("b", result.modifierKeys[1].key)
    }

    @Test
    fun resetModifierKeys_restoresDefault() {
        val emptyKeys = emptyList<ModifierKey>()
        val delegate = ModifierStateDelegate(TerminalState(modifierKeys = emptyKeys))
        val result = delegate.resetModifierKeys()
        assertTrue(result.modifierKeys.isNotEmpty())
        assertTrue(result.modifierKeys.size > 5)
    }

    @Test
    fun modifierStateOffIsDefault() {
        val state = TerminalState()
        assertEquals(ModifierState.Off, state.ctrlState)
        assertEquals(ModifierState.Off, state.altState)
    }

    @Test
    fun toggleScrollMode_flipsScrollActive() {
        val delegate = ModifierStateDelegate(TerminalState(scrollActive = false))
        val toggled = delegate.toggleScrollMode()
        assertTrue(toggled.scrollActive)
        val toggledBack = delegate.toggleScrollMode(toggled)
        assertFalse(toggledBack.scrollActive)
    }

    @Test
    fun setScrollActive_activatesAndDeactivates() {
        val delegate = ModifierStateDelegate(TerminalState())
        val activated = delegate.setScrollActive(true)
        assertTrue(activated.scrollActive)
        val deactivated = delegate.setScrollActive(false)
        assertFalse(deactivated.scrollActive)
    }
}

private class ModifierStateDelegate(
    private var state: TerminalState,
) {
    fun cycleCtrlState(currentState: TerminalState = state): TerminalState = currentState.copy(ctrlState = currentState.ctrlState.next())

    fun cycleAltState(currentState: TerminalState = state): TerminalState = currentState.copy(altState = currentState.altState.next())

    fun consumeOneShotModifiers(currentState: TerminalState = state): TerminalState {
        var newCtrl = currentState.ctrlState
        var newAlt = currentState.altState
        if (newCtrl == ModifierState.Once) newCtrl = ModifierState.Off
        if (newAlt == ModifierState.Once) newAlt = ModifierState.Off
        return currentState.copy(ctrlState = newCtrl, altState = newAlt)
    }

    fun consumePendingInput(currentState: TerminalState = state): PendingInputResult {
        val data = currentState.pendingInput
        return PendingInputResult(input = data, state = currentState.copy(pendingInput = null))
    }

    fun setModifierKeys(
        keys: List<ModifierKey>,
        currentState: TerminalState = state,
    ): TerminalState = currentState.copy(modifierKeys = keys)

    fun resetModifierKeys(currentState: TerminalState = state): TerminalState = currentState.copy(modifierKeys = defaultModifierKeys)

    fun toggleScrollMode(currentState: TerminalState = state): TerminalState = currentState.copy(scrollActive = !currentState.scrollActive)

    fun setScrollActive(
        active: Boolean,
        currentState: TerminalState = state,
    ): TerminalState = currentState.copy(scrollActive = active)
}

private data class PendingInputResult(
    val input: ByteArray?,
    val state: TerminalState,
)
