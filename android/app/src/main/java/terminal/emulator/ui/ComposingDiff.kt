package terminal.emulator.ui

/**
 * Composing-region reconciliation extracted from
 * TerminalSurface's InputConnection into a pure, unit-testable function.
 *
 * Mirrors warp WarpInputView.kt:587-615 (forwardComposingDiff) with the
 * three-way grow/contract/diverged handling torvox already had: the IME
 * reports an in-progress composing region (e.g. pinyin or swipe input);
 * each `setComposingText` delta must be translated into PTY edits
 * (backspaces by CODE POINT count + text to append) so the terminal's
 * buffer matches what the user sees in the IME.
 */
object ComposingDiff {
    /**
     * The edit needed to turn the previous composing buffer into [next]:
     * - grow (`ab` → `abc`): append the new suffix, no backspaces;
     * - contract (`a🎉b` → `a`): backspace the removed code points;
     * - diverged (`abc` → `xy`): backspace everything, then append;
     * - identical: no-op.
     *
     * Backspace counts use Unicode code points (not UTF-16 units) so an
     * emoji in the removed suffix is one backspace, never two.
     */
    data class Edit(
        val backspaces: Int,
        val append: String,
    ) {
        val isEmpty: Boolean get() = backspaces == 0 && append.isEmpty()
    }

    fun reconcile(
        previous: String,
        next: String,
    ): Edit {
        if (previous == next) return Edit(0, "")
        // Longest common prefix (by UTF-16 index — the split point is the
        // same for code points since the prefix is identical in both).
        var i = 0
        val maxI = minOf(previous.length, next.length)
        while (i < maxI && previous[i] == next[i]) i++

        val textToErase = if (previous.length > i) previous.substring(i) else ""
        val backspaces = textToErase.codePointCount(0, textToErase.length)
        val toAdd = if (next.length > i) next.substring(i) else ""
        return Edit(backspaces, toAdd)
    }
}
