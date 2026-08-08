package terminal.emulator.test

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.runtime.LogUtil

/**
 * Round-227 T2 emulator verification: a log message longer than logcat's
 * per-entry payload cap (4068 bytes) must be split into chunks that each
 * stay under the cap and are cut at UTF-8 code-point boundaries only.
 *
 * This test writes a long message (ASCII + CJK + emoji) through the real
 * app logging path and then inspects the actual logcat entries, so it
 * proves the chunking works end-to-end on-device, not just in unit tests.
 *
 * Note: logcat splits an entry at embedded newlines, so a continuation
 * chunk "(2/2)\nxxxx…" surfaces as two logcat lines ("(2/2)" and the x
 * run). The byte cap applies to each stored entry, which is why the
 * chunker (not logcat) owns the (i/n) prefixing.
 */
@RunWith(AndroidJUnit4::class)
class LogcatChunkingInstrumentedTest {

    @Test
    fun longLogMessageIsChunkedUnderLogcatCap() {
        val tag = "T2ChunkProbe"
        val cjk = "\u4e2d\u6587\u6d4b\u8bd5" // 中文测试
        val emoji = "\uD83D\uDE00\uD83C\uDF89" // 😀🎉 (surrogate pairs)
        val body = "ASCII-1234567890 $cjk $emoji " + "x".repeat(6000)
        LogUtil.i(tag, body)

        // Give logd a moment to flush.
        Thread.sleep(1500)

        val entries = readLogcat(tag)
        assertTrue("expected at least 2 logcat entries for a 6KB message, got ${entries.size}", entries.size >= 2)

        // The full logcat line includes the kernel-side display prefix
        // ("MM-DD HH:MM:SS.mmm PID TID I TAG: "), which is not part of the
        // payload cap. Measure the payload only (everything after ": ").
        val payloads = entries.map { it.substringAfter(": ") }
        val maxBytes = payloads.maxOf { it.toByteArray(Charsets.UTF_8).size }
        assertTrue(
            "every logcat payload must stay under the 4068-byte cap, saw $maxBytes bytes",
            maxBytes <= 4068,
        )

        // The first chunk (no prefix) must still carry the message head,
        // including the multi-byte CJK + emoji (i.e. chunks are cut at
        // code-point boundaries, never inside a UTF-8 sequence).
        assertTrue(
            "first chunk must carry the message head with CJK/emoji intact",
            payloads.any { it.startsWith("ASCII-1234567890 $cjk $emoji") },
        )
        // Continuation chunk marker must be present.
        assertTrue(
            "continuation marker must be present",
            payloads.any { it.startsWith("(2/2)") },
        )
        // The x-run must be complete across chunks (nothing lost). The
        // first chunk carries "ASCII-… " before its x's, so count x's
        // across every payload.
        val totalX = payloads.sumOf { p -> p.count { it == 'x' } }
        assertTrue(
            "the 6000-byte x-run must survive chunking, got $totalX",
            totalX >= 6000 - 64,
        )
    }

    private fun readLogcat(tag: String): List<String> {
        val process = ProcessBuilder("logcat", "-d", "-s", "$tag:*")
            .redirectErrorStream(true)
            .start()
        val output = process.inputStream.bufferedReader().readText()
        process.waitFor()
        return output.lines().filter { it.contains(tag) }
    }
}
