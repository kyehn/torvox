package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Smart-copy rules (round-225, Haven smartCopy:357-405). */
class SmartCopyTest {

    @Test
    fun `consistent borders detected across lines`() {
        val lines = listOf("│ left  │", "│ panel │", "│ body  │")
        val cols = SmartCopy.findConsistentBorderColumns(lines)
        assertEquals(setOf(0, 8), cols)
    }

    @Test
    fun `sporadic borders not detected`() {
        // Column 0 has │ in 2/3 lines (threshold 2) → border.
        // Column 4 has │ only in the last line (1/3 < 2) → not a border.
        val lines = listOf("│ a │", "b c d", "│ e  ")
        val cols = SmartCopy.findConsistentBorderColumns(lines)
        assertEquals(setOf(0), cols)
    }

    @Test
    fun `single non-blank line has no borders`() {
        assertEquals(emptySet<Int>(), SmartCopy.findConsistentBorderColumns(listOf("│ only │")))
    }

    @Test
    fun `panel content extracted between borders`() {
        val lines = listOf("│ left  │", "│ panel │")
        val out = SmartCopy.extractPanelContent(lines, setOf(0, 8), startCol = 3)
        assertEquals("left\npanel", out)
    }

    @Test
    fun `whole-panel extraction without right border`() {
        val lines = listOf("│ content  ", "│ more     ")
        val out = SmartCopy.extractPanelContent(lines, setOf(0), startCol = 2)
        assertEquals("content\nmore", out)
    }

    @Test
    fun `wrapped url rebuilt without newlines`() {
        val lines = listOf("https://example.com/very/", "long/path")
        val out =
            SmartCopy.rebuildWrappedUrl(
                lines,
                startRow = 0,
                startCol = 0,
                endRow = 1,
                endCol = "long/path".length - 1,
            )
        assertEquals("https://example.com/very/long/path", out)
    }

    @Test
    fun `non url multiselection keeps verbatim`() {
        val lines = listOf("ordinary", "text")
        val text =
            SmartCopy.smartCopyText(
                lines = lines,
                startRow = 0,
                startCol = 0,
                endRow = 1,
                endCol = 3,
                verbatim = "ordinary\ntext",
            )
        assertEquals("ordinary\ntext", text)
    }

    @Test
    fun `border strip wins over url rebuild`() {
        val lines = listOf("│ https://x.com/ab │", "│ cdef             │")
        val text =
            SmartCopy.smartCopyText(
                lines = lines,
                startRow = 0,
                startCol = 8,
                endRow = 1,
                endCol = 10,
                verbatim = "https://x.com/ab\ncdef",
            )
        // Border path must strip │ … │ and keep panel content.
        assertEquals("https://x.com/ab\ncdef", text)
    }

    @Test
    fun `looksLikeFullUrl gates url shapes`() {
        assertTrue(SmartCopy.looksLikeFullUrl("https://example.com/path"))
        assertTrue(SmartCopy.looksLikeFullUrl("www.example.com/path"))
        assertTrue(!SmartCopy.looksLikeFullUrl("ordinary text"))
        assertTrue(!SmartCopy.looksLikeFullUrl("not-a-url"))
    }
}
