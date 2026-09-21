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
    fun `small writes send immediately without flush`() {
        // 小提交直发：单字符量级不再驻留缓冲，延迟与退格直连对齐。
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 16)
        buffer.write(byteArrayOf(1, 2, 3))
        awaitSize(sent, 1)
        assertEquals(1, sent.size)
        assertArrayEquals(byteArrayOf(1, 2, 3), sent[0])
        buffer.close()
    }

    @Test
    fun `multi-codepoint composition commit sends immediately without flush`() {
        // 组合提交直发：编码后 12 字节（4 个汉字）超过单码点量级，仍不等待帧回调。
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        val commit = "你好世界".toByteArray(Charsets.UTF_8)
        buffer.write(commit)
        awaitSize(sent, 1)
        assertEquals(1, sent.size)
        assertArrayEquals(commit, sent[0])
        buffer.close()
    }

    @Test
    fun `backspace then composition commit sends promptly in order`() {
        // 退格（0x08）后紧接的 12 字节组合提交：两者都立即发送、顺序保持，
        // 帧等待延迟不累积。真机退格走 writeToPty 直连，此处验证其后提交不被钳制。
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        val commit = "你好世界".toByteArray(Charsets.UTF_8)
        buffer.write(byteArrayOf(0x08))
        buffer.write(commit)
        awaitSize(sent, 2)
        assertEquals(2, sent.size)
        assertArrayEquals(byteArrayOf(0x08), sent[0])
        assertArrayEquals(commit, sent[1])
        buffer.close()
    }

    @Test
    fun `large writes stay buffered until flush`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        buffer.write(ByteArray(100) { 7 })
        assertEquals(0, sent.size)
        buffer.flush()
        awaitSize(sent, 1)
        assertEquals(1, sent.size)
        assertEquals(100, sent[0].size)
        buffer.close()
    }

    @Test
    fun `small write drains buffered bytes first preserving order`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        buffer.write(ByteArray(100) { 7 })
        buffer.write(byteArrayOf(1, 2, 3))
        awaitSize(sent, 2)
        assertEquals(2, sent.size)
        assertEquals(100, sent[0].size)
        assertArrayEquals(byteArrayOf(1, 2, 3), sent[1])
        buffer.close()
    }

    @Test
    fun `oversized write flushes buffered input first`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        buffer.write(ByteArray(100) { 7 })
        buffer.write(ByteArray(200) { 9 })
        awaitSize(sent, 2)
        assertEquals(2, sent.size)
        assertEquals(100, sent[0].size)
        assertEquals(200, sent[1].size)
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
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        buffer.write(ByteArray(100) { 7 })
        buffer.reset()
        buffer.flush()
        awaitSize(sent, 0)
        assertEquals(0, sent.size)
        buffer.close()
    }

    @Test
    fun `close flushes buffered bytes before shutdown`() {
        val sent = mutableListOf<ByteArray>()
        val buffer = InputBatchBuffer.forTest({ sent.add(it) }, capacity = 128)
        buffer.write(ByteArray(100) { 7 })
        // No flush yet — the Choreographer callback never fires in tests
        // (useChoreographer=false), so the bytes are still buffered.
        buffer.close()
        awaitSize(sent, 1)
        val flushed = sent.flatMap { it.toList() }.toByteArray()
        assertArrayEquals(ByteArray(100) { 7 }, flushed)
    }
}