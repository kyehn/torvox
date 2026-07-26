package io.term

import io.term.ui.ModifierState
import io.term.ui.defaultModifierKeys
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalStateTest {
    @Test
    fun defaultTerminalStateIsEmpty() {
        val state = TerminalState()
        assertEquals(0L, state.sessionId)
        assertFalse(state.isRunning)
        assertEquals("Terminal", state.title)
        assertEquals(emptyList<SessionInfo>(), state.sessions)
        assertEquals(0L, state.activeSessionId)
        assertNull(state.pendingInput)
        assertEquals(ModifierState.Off, state.ctrlState)
        assertEquals(ModifierState.Off, state.altState)
        assertFalse(state.scrollActive)
    }

    @Test
    fun defaultSelectionIsInactive() {
        val state = TerminalState()
        assertFalse(state.selection.active)
        assertNull(state.selection.start)
        assertNull(state.selection.end)
        assertEquals(SelectionMode.Char, state.selection.mode)
        assertEquals("", state.selection.selectedText)
    }

    @Test
    fun defaultModifierKeysArePopulated() {
        val state = TerminalState()
        assertTrue(state.modifierKeys.isNotEmpty())
        assertEquals(defaultModifierKeys.size, state.modifierKeys.size)
        assertEquals(state.modifierKeys, defaultModifierKeys)
    }

    @Test
    fun copyPreservesUnchangedFields() {
        val original = TerminalState(sessionId = 7L, title = "custom")
        val copy = original.copy(ctrlState = ModifierState.Locked)
        assertEquals(7L, copy.sessionId)
        assertEquals("custom", copy.title)
        assertEquals(ModifierState.Locked, copy.ctrlState)
        assertEquals(ModifierState.Off, copy.altState)
    }

    @Test
    fun sessionInfoEqualityIncludesAllFields() {
        val firstSession = SessionInfo(id = 1L, title = "first")
        val renamedSession = SessionInfo(id = 1L, title = "renamed")
        val secondSession = SessionInfo(id = 2L, title = "first")
        assertNotEquals("data class: different title should differ", firstSession, renamedSession)
        assertNotEquals("data class: different id should differ", firstSession, secondSession)
    }

    @Test
    fun selectionStateCopyKeepsAnchor() {
        val anchor = SelectionAnchor(row = 2, col = 5)
        val active = SelectionState(active = true, start = anchor, end = anchor)
        val copy = active.copy(selectedText = "hello")
        assertEquals(anchor, copy.start)
        assertEquals(anchor, copy.end)
        assertEquals("hello", copy.selectedText)
    }

    @Test
    fun selectionModeEnumHasExpectedVariants() {
        val variants = SelectionMode.entries
        assertEquals(5, variants.size)
        assertTrue(variants.contains(SelectionMode.Char))
        assertTrue(variants.contains(SelectionMode.Word))
        assertTrue(variants.contains(SelectionMode.Line))
        assertTrue(variants.contains(SelectionMode.Block))
        assertTrue(variants.contains(SelectionMode.Semantic))
    }

    @Test
    fun modifierKeysContainExpectedTypes() {
        val labels = defaultModifierKeys.map { it.key }
        assertTrue(labels.contains("ctrl"))
        assertTrue(labels.contains("alt"))
        assertTrue(labels.contains("esc"))
        assertTrue(labels.contains("tab"))
    }
}
