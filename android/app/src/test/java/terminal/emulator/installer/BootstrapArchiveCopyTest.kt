package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.InterruptedIOException
import java.io.OutputStream

/**
 * 在线下载与离线 SAF 选择共用同一条复制路径：此处断言复制完整、进度单调、
 * 取消即停。
 */
class BootstrapArchiveCopyTest {
    @Test
    fun `stream within the cap is copied in full`() {
        val payload = ByteArray(64 * 1024) { (it % 251).toByte() }
        val sink = ByteArrayOutputStream()
        val reported = mutableListOf<Long>()

        copyBootstrapArchive(
            input = ByteArrayInputStream(payload),
            output = sink,
            onCopied = { reported.add(it) },
        )

        assertEquals(payload.size.toLong(), reported.last())
        assertTrue("上报进度必须单调不减", reported.zipWithNext().all { (before, after) -> after >= before })
        assertEquals(payload.toList(), sink.toByteArray().toList())
    }

    @Test
    fun `aborting copy stops before writing anything`() {
        val sink = CountingOutputStream()
        try {
            copyBootstrapArchive(
                input = ByteArrayInputStream(ByteArray(BootstrapInstaller.COPY_BUFFER_SIZE)),
                output = sink,
                shouldAbort = { true },
            )
            fail("取消必须中断复制")
        } catch (expected: InterruptedIOException) {
            assertEquals("取消时不得写入任何字节", 0L, sink.written)
        }
    }

    /** 只统计写入量而不持有内容。 */
    private class CountingOutputStream : OutputStream() {
        var written = 0L
            private set

        override fun write(byteValue: Int) {
            written++
        }

        override fun write(buffer: ByteArray, offset: Int, length: Int) {
            written += length
        }
    }
}
