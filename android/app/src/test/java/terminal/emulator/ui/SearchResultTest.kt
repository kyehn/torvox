package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class SearchResultTest {
    @Test
    fun `SearchResult holds grid coordinates`() {
        val r = SearchResult(lineIndex = 7, startIndex = 3, endIndex = 9)
        assertEquals(7, r.lineIndex)
        assertEquals(3, r.startIndex)
        assertEquals(9, r.endIndex)
    }

    @Test
    fun `SearchResult is a data class`() {
        assertEquals(SearchResult(1, 2, 3), SearchResult(1, 2, 3))
        assertEquals(
            SearchResult(1, 2, 3).copy(endIndex = 4),
            SearchResult(1, 2, 4),
        )
    }
}
