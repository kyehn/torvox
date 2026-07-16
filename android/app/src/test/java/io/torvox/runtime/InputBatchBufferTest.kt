package io.torvox.runtime

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class InputBatchBufferTest {
    @Test
    fun single_write_flushes_on_choreographer() {
        val flushed = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ data -> flushed.add(data) })
        buffer.write("hello".toByteArray())
        buffer.flush()
        assertEquals(1, flushed.size)
        assertArrayEquals("hello".toByteArray(), flushed[0])
    }

    @Test
    fun multiple_writes_batch_into_single_flush() {
        val flushed = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ data -> flushed.add(data) })
        buffer.write("a".toByteArray())
        buffer.write("b".toByteArray())
        buffer.write("c".toByteArray())
        buffer.flush()
        assertEquals(1, flushed.size)
        assertArrayEquals("abc".toByteArray(), flushed[0])
    }

    @Test
    fun empty_buffer_flush_does_nothing() {
        var flushCount = 0
        val buffer = InputBatchBuffer.forTest({ flushCount++ })
        buffer.flush()
        assertEquals(0, flushCount)
    }

    @Test
    fun large_write_exceeds_capacity_flushes_immediately() {
        val flushed = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ data -> flushed.add(data) }, capacity = 16)
        val large = ByteArray(32) { 'x'.code.toByte() }
        buffer.write(large)
        buffer.flush()
        assertEquals(1, flushed.size)
        assertArrayEquals(large, flushed[0])
    }

    @Test
    fun exact_capacity_write_works() {
        val flushed = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ data -> flushed.add(data) }, capacity = 16)
        val exact = "1234567890123456".toByteArray()
        assertEquals(16, exact.size)
        buffer.write(exact)
        buffer.flush()
        assertEquals(1, flushed.size)
        assertArrayEquals(exact, flushed[0])
    }

    @Test
    fun multiple_flushes_clear_between() {
        val flushed = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ data -> flushed.add(data) })
        buffer.write("first".toByteArray())
        buffer.flush()
        buffer.write("second".toByteArray())
        buffer.flush()
        assertEquals(2, flushed.size)
        assertArrayEquals("first".toByteArray(), flushed[0])
        assertArrayEquals("second".toByteArray(), flushed[1])
    }

    @Test
    fun reset_clears_pending() {
        var flushCount = 0
        val buffer = InputBatchBuffer.forTest({ flushCount++ })
        buffer.write("data".toByteArray())
        buffer.reset()
        buffer.flush()
        assertEquals(0, flushCount)
    }

    @Test
    fun write_after_reset_still_works() {
        val flushed = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ data -> flushed.add(data) })
        buffer.write("first".toByteArray())
        buffer.reset()
        buffer.write("second".toByteArray())
        buffer.flush()
        assertEquals(1, flushed.size)
        assertArrayEquals("second".toByteArray(), flushed[0])
    }
}
