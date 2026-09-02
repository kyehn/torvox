package terminal.emulator.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * table-driven tests for [isWhitespaceCell] — the paste-only blank-target classification (spec
 * text-selection "空白处长按仅粘贴菜单").
 *
 * Boundary semantics pinned here:
 * - a null row (blank scrollback line) is blank;
 * - a whitespace cell is blank;
 * - ANY column at or past the end of the line is blank — this is root-cause A1: the old `col <
 *   line.length` conjunct classified end-of-line columns as TEXT, so long-pressing right of the
 *   prompt showed PASTE;
 * - a printable cell is not blank.
 */
class IsWhitespaceCellTest {

    @Test
    fun nullRowIsBlank() {
        assertTrue(isWhitespaceCell(null, col = 0))
        assertTrue(isWhitespaceCell(null, col = 40))
    }

    @Test
    fun whitespaceCellIsBlank() {
        assertTrue(isWhitespaceCell("ls  ", col = 2))
        assertTrue(isWhitespaceCell("ls  ", col = 3))
        assertTrue(isWhitespaceCell("\ttab", col = 0))
    }

    @Test
    fun endOfLineColumnIsBlank() {
        // Exactly one past the last character.
        assertTrue(isWhitespaceCell("abc", col = 3))
        // Far beyond the end (grid extends past the text).
        assertTrue(isWhitespaceCell("abc", col = 79))
    }

    @Test
    fun printableCellIsNotBlank() {
        assertFalse(isWhitespaceCell("abc", col = 0))
        assertFalse(isWhitespaceCell("abc", col = 2))
        assertFalse(isWhitespaceCell("$ ", col = 0))
    }

    @Test
    fun promptTailScenarioClassifiesAsBlank() {
        // The reported symptom: shell prompt "$ " then long-press on the empty
        // area right of it must be paste-only, not a text selection.
        val promptLine = "kyehn@host:~$ "
        val firstColumnPastText = promptLine.length
        assertTrue(isWhitespaceCell(promptLine, firstColumnPastText))
        assertTrue(isWhitespaceCell(promptLine, firstColumnPastText + 10))
        // The space inside the prompt is also blank.
        assertTrue(isWhitespaceCell(promptLine, promptLine.length - 1))
    }
}
