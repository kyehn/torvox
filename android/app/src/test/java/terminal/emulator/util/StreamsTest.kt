package terminal.emulator.util

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.ByteArrayInputStream

class StreamsTest {
    @Test
    fun readsShorterStreamFully() {
        val data = byteArrayOf(1, 2, 3)
        assertArrayEquals(data, ByteArrayInputStream(data).readAtMost(8))
    }

    @Test
    fun stopsAtLimit() {
        val data = ByteArray(8) { it.toByte() }
        val read = ByteArrayInputStream(data).readAtMost(4)
        assertEquals(4, read.size)
        assertArrayEquals(data.copyOf(4), read)
    }
}
