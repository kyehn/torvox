package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class InputCoalescerTest {
    @Test
    fun `send forwards bytes verbatim to sink`() {
        val received = mutableListOf<ByteArray>()
        val coalescer = InputCoalescer { received.add(it) }
        coalescer.send(byteArrayOf(0x61, 0x62))
        assertEquals(1, received.size)
        assertTrue(received[0].contentEquals(byteArrayOf(0x61, 0x62)))
    }

    @Test
    fun `composing text lifecycle`() {
        val coalescer = InputCoalescer { }
        assertNull(coalescer.getComposingText())
        assertFalse(coalescer.isComposing())
        coalescer.updateComposingText("你")
        assertEquals("你", coalescer.getComposingText())
        assertTrue(coalescer.isComposing())
        coalescer.clearComposing()
        assertNull(coalescer.getComposingText())
    }

    @Test
    fun `reset clears composing state`() {
        val coalescer = InputCoalescer { }
        coalescer.updateComposingText("abc")
        coalescer.reset()
        assertNull(coalescer.getComposingText())
        assertFalse(coalescer.isComposing())
    }
}
