package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SearchResultTest {
    // --- isNarrowingDown tests (GNOME Console kgx-tab.c:191-250 equivalent) ---

    @Test
    fun `narrowing down - prefix deletion`() {
        // User had "foobar" and deleted last chars to "foo"
        assertTrue(SearchResult.isNarrowingDown(query = "foo", previousQuery = "foobar"))
    }

    @Test
    fun `narrowing down - suffix deletion via contains not startsWith`() {
        // User had "foobar" and deleted first chars to "bar"
        // OLD behavior (startsWith): false → index reset to 0
        // NEW behavior (contains): true → preserves index
        assertTrue(SearchResult.isNarrowingDown(query = "bar", previousQuery = "foobar"))
    }

    @Test
    fun `not narrowing - middle deletion (not a substring)`() {
        // User had "foobar" and deleted middle 'b' → "foar"
        // "foobar" does NOT contain "foar" as a substring — this is NOT narrowing
        // This is a fundamental difference from edit-distance: narrowing_down only
        // preserves index when the shorter query is a literal substring of the longer.
        assertFalse(SearchResult.isNarrowingDown(query = "foar", previousQuery = "foobar"))
    }

    @Test
    fun `narrowing down - middle substring exists`() {
        // "foobar" contains "oob" at index 1-3 → narrowing
        assertTrue(SearchResult.isNarrowingDown(query = "oob", previousQuery = "foobar"))
    }

    @Test
    fun `narrowing down - single char shorter`() {
        assertTrue(SearchResult.isNarrowingDown(query = "foob", previousQuery = "foobar"))
    }

    @Test
    fun `not narrowing - same length`() {
        assertFalse(SearchResult.isNarrowingDown(query = "foo", previousQuery = "foo"))
    }

    @Test
    fun `not narrowing - query longer (expansion)`() {
        assertFalse(SearchResult.isNarrowingDown(query = "foobar", previousQuery = "foo"))
    }

    @Test
    fun `not narrowing - empty query`() {
        assertFalse(SearchResult.isNarrowingDown(query = "", previousQuery = "foobar"))
    }

    @Test
    fun `not narrowing - empty previous query`() {
        assertFalse(SearchResult.isNarrowingDown(query = "foo", previousQuery = ""))
    }

    @Test
    fun `not narrowing - both empty`() {
        assertFalse(SearchResult.isNarrowingDown(query = "", previousQuery = ""))
    }

    @Test
    fun `not narrowing - unrelated strings`() {
        assertFalse(SearchResult.isNarrowingDown(query = "xyz", previousQuery = "foobar"))
    }

    @Test
    fun `not narrowing - partially overlapping but not contained`() {
        // "foobaz" does not contain "foobx"
        assertFalse(SearchResult.isNarrowingDown(query = "foobx", previousQuery = "foobaz"))
    }

    @Test
    fun `not narrowing - case mismatch prevents substring match`() {
        // "Foobar" does not contain "foo" (case-sensitive)
        assertFalse(SearchResult.isNarrowingDown(query = "foo", previousQuery = "Foobar"))
    }

    @Test
    fun `narrowing down - CJK characters`() {
        assertTrue(SearchResult.isNarrowingDown(query = "测试", previousQuery = "测试字符串"))
    }

    @Test
    fun `narrowing down - emoji`() {
        assertTrue(SearchResult.isNarrowingDown(query = "😀", previousQuery = "😀🎉"))
    }

    @Test
    fun `not narrowing - CJK unrelated`() {
        assertFalse(SearchResult.isNarrowingDown(query = "你好", previousQuery = "测试"))
    }
}
