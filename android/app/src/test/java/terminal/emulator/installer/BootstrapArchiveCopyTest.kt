package terminal.emulator.installer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.io.InterruptedIOException
import java.io.OutputStream

/**
 * 在线下载与离线 SAF 选择共用同一条带硬上限的复制路径：任一路径都不得把
 * 无界流写进 cacheDir。此处断言上限被真正执行，而非只是常量存在。
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
    fun `stream past the cap is rejected instead of filling the partition`() {
        val sink = CountingOutputStream()

        try {
            copyBootstrapArchive(
                input = ZeroStream(BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES + 1),
                output = sink,
            )
            fail("超过上限的归档必须失败")
        } catch (expected: IOException) {
            assertTrue(
                "错误消息须指明上限：${expected.message}",
                expected.message.orEmpty().contains(BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES.toString()),
            )
        }
        assertTrue(
            "写入量须硬性截断在上限之内，实际 ${sink.written}",
            sink.written <= BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES,
        )
    }

    @Test
    fun `aborting copy stops before writing anything`() {
        val sink = CountingOutputStream()
        try {
            copyBootstrapArchive(
                input = ZeroStream(BootstrapInstaller.MAX_BOOTSTRAP_SIZE_BYTES),
                output = sink,
                shouldAbort = { true },
            )
            fail("取消必须中断复制")
        } catch (expected: InterruptedIOException) {
            assertEquals("取消时不得写入任何字节", 0L, sink.written)
        }
    }

    /** 按声明长度生成零字节流，使上限测试无需分配真实缓冲。 */
    private class ZeroStream(private var remaining: Long) : InputStream() {
        override fun read(): Int = if (remaining-- <= 0) -1 else 0

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
            if (remaining <= 0) return -1
            val count = minOf(length.toLong(), remaining).toInt()
            remaining -= count
            return count
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
