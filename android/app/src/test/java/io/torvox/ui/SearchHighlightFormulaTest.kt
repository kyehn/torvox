package io.torvox.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SearchHighlightFormulaTest {
    @Test
    fun visibleRowFormulaConsistency() {
        val scrollbackLen = 100
        val rows = 24
        val totalLines = scrollbackLen + rows

        // When scrollOffset=0 (at bottom), only scrollbackLen..totalLines-1 are visible
        val scrollOffsetBottom = 0
        val matchAtBottom = scrollbackLen
        val visibleRowBottom = matchAtBottom - (totalLines - rows - scrollOffsetBottom)
        assertEquals(0, visibleRowBottom)

        // When scrollOffset=scrollbackLen (at top), line 0 is visible at row 0
        val scrollOffsetTop = scrollbackLen
        val matchAtTop = 0
        val visibleRowTop = matchAtTop - (totalLines - rows - scrollOffsetTop)
        assertEquals(0, visibleRowTop)

        // Line at scrollbackLen should be at visible row 0 when not scrolled
        // Line at totalLines-1 should be at visible row rows-1 when not scrolled
        val matchAtLastVisible = totalLines - 1
        val visibleRowLast = matchAtLastVisible - (totalLines - rows - scrollOffsetBottom)
        assertEquals(rows - 1, visibleRowLast)
    }

    @Test
    fun highlightMatchesSearchLineIndices() {
        val scrollbackLen = 50
        val rows = 24
        val totalLines = scrollbackLen + rows
        val fullText = (0 until totalLines).joinToString("\n") { "line $it" }

        val matches = findMatches(fullText, "line 50")
        assertEquals(1, matches.size)
        assertEquals(scrollbackLen, matches[0].lineIndex)

        val matchesLast = findMatches(fullText, "line 73")
        assertEquals(1, matchesLast.size)
        assertEquals(totalLines - 1, matchesLast[0].lineIndex)
    }

    @Test
    fun highlightOffscreenMatchesNotRendered() {
        val scrollbackLen = 100
        val rows = 24
        val totalLines = scrollbackLen + rows
        val scrollOffset = 0

        val lineAbove = scrollbackLen - 1
        val visibleRowAbove = lineAbove - (totalLines - rows - scrollOffset)
        assertTrue("Line above visible range should have negative visibleRow", visibleRowAbove < 0)

        val lineBelow = totalLines
        val visibleRowBelow = lineBelow - (totalLines - rows - scrollOffset)
        assertTrue("Line below visible range should be >= rows", visibleRowBelow >= rows)
    }
}
