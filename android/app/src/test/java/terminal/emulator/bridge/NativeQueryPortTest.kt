package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Test

class NativeQueryPortTest {
    @Test
    fun `empty native result yields no matches`() {
        assertEquals(emptyList<Triple<Int, Int, Int>>(), searchMatches(IntArray(0)))
    }

    @Test
    fun `single flat triple is unpacked in order`() {
        val matches = searchMatches(intArrayOf(5, 10, 15))
        assertEquals(listOf(Triple(5, 10, 15)), matches)
    }

    @Test
    fun `multiple flat triples keep native order`() {
        val matches = searchMatches(intArrayOf(0, 1, 2, 3, 4, 5, 6, 7, 8))
        assertEquals(listOf(Triple(0, 1, 2), Triple(3, 4, 5), Triple(6, 7, 8)), matches)
    }
}
