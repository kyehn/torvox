package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * 超长查询 MUST NOT 被截断成另一个串再送原生：超长直接判无命中。
 */
class SearchQueryCoerceTest {
    @Test
    fun `short query passes through unchanged`() {
        assertEquals("hello", coerceSearchQuery("hello", 128))
    }

    @Test
    fun `query at limit passes through unchanged`() {
        val query = "a".repeat(128)
        assertEquals(query, coerceSearchQuery(query, 128))
    }

    @Test
    fun `overlong query returns null instead of truncated`() {
        assertNull(coerceSearchQuery("a".repeat(129), 128))
    }

    @Test
    fun `empty query passes through`() {
        assertEquals("", coerceSearchQuery("", 128))
    }

    @Test
    fun `astral chars count as one codepoint like native`() {
        val query = "😀".repeat(65)
        assertEquals(query, coerceSearchQuery(query, 128))
    }

    @Test
    fun `negative limit fails fast instead of silent no hit`() {
        assertThrows(IllegalArgumentException::class.java) { coerceSearchQuery("", -1) }
    }
}
