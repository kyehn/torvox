package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class NativeQueryPortTest {
    @Test
    fun `parseSearchMatches returns empty list for null-ish input`() {
        assertTrue(parseSearchMatches("[]").isEmpty())
        assertTrue(parseSearchMatches("").isEmpty())
    }

    @Test
    fun `parseSearchMatches parses native row objects`() {
        val result = parseSearchMatches("""[{"row":5,"start_col":10,"end_col":15}]""")
        assertEquals(listOf(Triple(5, 10, 15)), result)
    }

    @Test
    fun `parseSearchMatches handles multiple matches and defaults`() {
        val result = parseSearchMatches(
            """[{"row":0,"start_col":1,"end_col":2},{"row":3,"start_col":4,"end_col":5}]""",
        )
        assertEquals(listOf(Triple(0, 1, 2), Triple(3, 4, 5)), result)
        assertTrue("all-default entry is filtered (end <= start)", parseSearchMatches("""[{}]""").isEmpty())
    }

    @Test
    fun `parseSearchMatches is lenient on malformed json`() {
        assertTrue(parseSearchMatches("not json").isEmpty())
        assertTrue(parseSearchMatches("""[{"row":"x"}]""").isEmpty())
        assertTrue(parseSearchMatches("""{}}""").isEmpty())
    }

    @Test
    fun `parseSearchMatches tolerates unknown extra fields`() {
        val result = parseSearchMatches(
            """[{"row":1,"start_col":2,"end_col":3,"future":true}]""",
        )
        assertEquals(listOf(Triple(1, 2, 3)), result)
    }
}

class NativeQueryPortValidationTest {
    @Test
    fun `drops matches with impossible ranges`() {
        val json = """[{"row":0,"start_col":3,"end_col":2},{"row":1,"start_col":0,"end_col":5},{"row":-1,"start_col":0,"end_col":1}]"""
        val matches = parseSearchMatches(json)
        assertEquals("only the valid row=1 match survives", 1, matches.size)
        assertEquals(Triple(1, 0, 5), matches[0])
    }
}
