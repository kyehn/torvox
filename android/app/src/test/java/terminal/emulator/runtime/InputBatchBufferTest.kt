package terminal.emulator.runtime

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class InputBatchBufferTest {
    /** Sink delivery is async (single daemon sender thread); poll until delivered. */
    private fun awaitSize(sent: MutableList<ByteArray>, expected: Int) {
        val deadline = System.currentTimeMillis() + 2_000
        while (sent.size < expected && System.currentTimeMillis() < deadline) {
            Thread.sleep(5)
        }
    }

    @Test
    fun `small writes stay buffered until flush`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 16)
        buffer.write(byteArrayOf(1, 2, 3))
        assertEquals(0, sent.size)
        buffer.flush()
        awaitSize(sent, 1)
        assertEquals(1, sent.size)
        assertArrayEquals(byteArrayOf(1, 2, 3), sent[0])
        buffer.close()
    }

    @Test
    fun `oversized write flushes buffered input first`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 8)
        buffer.write(byteArrayOf(1, 2))
        buffer.write(ByteArray(20) { 9 })
        awaitSize(sent, 2)
        assertEquals(2, sent.size)
        assertArrayEquals(byteArrayOf(1, 2), sent[0])
        assertEquals(20, sent[1].size)
        buffer.close()
    }

    @Test
    fun `flush with empty buffer sends nothing`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) })
        buffer.flush()
        assertEquals(0, sent.size)
        buffer.close()
    }

    @Test
    fun `reset clears pending bytes`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 16)
        buffer.write(byteArrayOf(1, 2, 3))
        buffer.reset()
        buffer.flush()
        awaitSize(sent, 0)
        assertEquals(0, sent.size)
        buffer.close()
    }
}
