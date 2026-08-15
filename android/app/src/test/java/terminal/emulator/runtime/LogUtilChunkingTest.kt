package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LogUtilChunkingTest {
    private val tag = "Runtime"

    @Test
    fun `short message stays single chunk`() {
        assertEquals(listOf("hello"), LogUtil.chunkMessage(tag, "hello"))
    }

    @Test
    fun `empty message stays single chunk`() {
        assertEquals(listOf(""), LogUtil.chunkMessage(tag, ""))
    }

    @Test
    fun `message at exact budget stays single chunk`() {
        val budget = LogUtil.maxEntrySize(tag.length)
        val msg = "x".repeat(budget)
        val chunks = LogUtil.chunkMessage(tag, msg)
        assertEquals(1, chunks.size)
        assertEquals(budget, chunks[0].length)
    }

    @Test
    fun `over budget splits and every chunk fits`() {
        val budget = LogUtil.maxEntrySize(tag.length)
        val msg = "y".repeat(budget * 3 + 17)
        val chunks = LogUtil.chunkMessage(tag, msg)
        assertTrue("expected >=3 chunks, got ${chunks.size}", chunks.size >= 3)
        assertTrue(chunks[0].length <= budget)
        chunks.drop(1).forEach { chunk ->
            assertTrue(
                "chunk len ${chunk.length} exceeds budget $budget",
                chunk.length <= budget,
            )
        }
        // Reassemble (strip "(i/n)\n" prefixes) and compare with original.
        val reassembled = buildString {
            append(chunks[0])
            for ((i, chunk) in chunks.withIndex().drop(1)) {
                val prefix = "(${i + 1}/${chunks.size})\n"
                assertTrue("missing prefix in $chunk", chunk.startsWith(prefix))
                append(chunk.removePrefix(prefix))
            }
        }
        assertEquals(msg, reassembled)
    }

    @Test
    fun `multibyte characters are never split`() {
        val budget = LogUtil.maxEntrySize("t".length)
        val unit = "中" // 3 bytes in UTF-8
        val msg = unit.repeat(budget / unit.length + 2)
        val chunks = LogUtil.chunkMessage("t", msg)
        chunks.forEach { chunk ->
            // Every chunk is made of whole 3-byte chars.
            assertEquals(0, chunk.toByteArray(Charsets.UTF_8).size % unit.toByteArray(Charsets.UTF_8).size)
        }
        val concat = buildString {
            append(chunks[0])
            for ((i, chunk) in chunks.withIndex().drop(1)) {
                val prefix = "(${i + 1}/${chunks.size})\n"
                append(chunk.removePrefix(prefix))
            }
        }
        assertEquals(msg, concat)
    }

    @Test
    fun `chunk boundary prefers newline`() {
        val budget = LogUtil.maxEntrySize("t".length)
        val effective = budget - 8
        val line1 = "a".repeat(effective - 4)
        val msg = "$line1\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        val chunks = LogUtil.chunkMessage("t", msg)
        assertTrue("expected >=2 chunks, got ${chunks.size}", chunks.size >= 2)
        assertTrue("first chunk should end at newline", chunks[0].endsWith("\n"))
    }

    @Test
    fun `huge tag still yields usable floor`() {
        assertEquals(64, LogUtil.maxEntrySize(1_000_000))
        // Tag-less floor: 4068 payload − 32 prefix overhead − 4 safety margin.
        assertEquals(4032, LogUtil.maxEntrySize(0))
    }

    @Test
    fun `continuation prefix format matches native`() {
        // budget("t") = 4068-32-1-4 = 4031; 5000 chars forces multiple chunks.
        val chunks = LogUtil.chunkMessage("t", "a".repeat(5000))
        assertTrue(chunks.size > 1)
        assertTrue(
            "prefix must match (i/n) format: ${chunks[1].take(8)}",
            chunks[1].matches(Regex("^\\(\\d+/\\d+\\)\\n.*", RegexOption.DOT_MATCHES_ALL)),
        )
    }

    // ──  audit fix: UTF-8 byte budget (spec d2) ──────────────

    @Test
    fun `cjk message chunks never exceed 4068 bytes`() {
        // Regression: the pre-fix implementation counted UTF-16 code units,
        // so 4047 "中" (3 bytes each) filled one chunk with 12141 bytes —
        // logcat truncates it, violating spec d2.
        val budget = LogUtil.maxEntrySize("t".toByteArray(Charsets.UTF_8).size)
        val unit = "中"
        val msg = unit.repeat(budget / unit.toByteArray(Charsets.UTF_8).size + 2)
        val chunks = LogUtil.chunkMessage("t", msg)
        assertTrue("expected >=2 chunks, got ${chunks.size}", chunks.size >= 2)
        chunks.forEachIndexed { i, chunk ->
            val bytes = chunk.toByteArray(Charsets.UTF_8).size
            assertTrue(
                "chunk $i has $bytes bytes, budget $budget",
                bytes <= budget,
            )
            // Never split a multi-byte char.
            assertEquals(0, bytes % unit.toByteArray(Charsets.UTF_8).size)
        }
        // Reassembled (minus prefixes) equals the original.
        val concat = buildString {
            append(chunks[0])
            for ((i, chunk) in chunks.withIndex().drop(1)) {
                append(chunk.removePrefix("(${i + 1}/${chunks.size})\n"))
            }
        }
        assertEquals(msg, concat)
    }

    @Test
    fun `emoji surrogate pairs are never split and chunks fit budget`() {
        val budget = LogUtil.maxEntrySize("t".toByteArray(Charsets.UTF_8).size)
        // 😀 = U+1F600, 4 UTF-8 bytes, 2 UTF-16 code units.
        val emoji = "\uD83D\uDE00"
        val msg = emoji.repeat(budget / 4 + 5)
        val chunks = LogUtil.chunkMessage("t", msg)
        assertTrue("expected >=2 chunks, got ${chunks.size}", chunks.size >= 2)
        chunks.forEachIndexed { i, chunk ->
            val bytes = chunk.toByteArray(Charsets.UTF_8).size
            assertTrue(
                "chunk $i has $bytes bytes, budget $budget",
                bytes <= budget,
            )
            // Every chunk is valid UTF-8 with no lone surrogates.
            val decoded = chunk.toByteArray(Charsets.UTF_8).toString(Charsets.UTF_8)
            assertEquals(chunk, decoded)
        }
    }

    @Test
    fun `hundred-plus chunks keep prefix within budget`() {
        // "(100/100)\n" is 10 bytes > the 8-byte estimate; the dynamic
        // prefix re-split must keep every continuation chunk within budget.
        val budget = LogUtil.maxEntrySize("t".toByteArray(Charsets.UTF_8).size)
        val msg = "x".repeat((budget - 8) * 120) // ~120 chunks
        val chunks = LogUtil.chunkMessage("t", msg)
        assertTrue("expected >=100 chunks, got ${chunks.size}", chunks.size >= 100)
        chunks.drop(1).forEachIndexed { i, chunk ->
            assertTrue(
                "chunk ${i + 1} exceeds budget: ${chunk.length} > $budget",
                chunk.length <= budget,
            )
        }
        val prefix = "(${chunks.size}/${chunks.size})\n"
        assertTrue("prefix length must match: ${prefix.length}", prefix.length > 8)
    }
}

class WrapTermuxExecTest {
    @org.junit.Test
    fun prefixBinaryIsWrappedInLinker() {
        val argv = listOf("/data/user/0/com.termux/files/usr/bin/echo", "hello")
        val wrapped = terminal.emulator.runtime.wrapTermuxExec(argv, "/data/user/0/com.termux/files/usr")
        // Host JVM has no /system/bin/linker64 (that is Android-only), so
        // accept either linker name; the argv shape must be linker+exe+args.
        org.junit.Assert.assertTrue(
            "first arg must be the system linker, got ${wrapped[0]}",
            wrapped[0] == "/system/bin/linker64" || wrapped[0] == "/system/bin/linker",
        )
        org.junit.Assert.assertEquals(
            listOf(wrapped[0], "/data/user/0/com.termux/files/usr/bin/echo", "hello"),
            wrapped,
        )
    }

    @org.junit.Test
    fun systemBinaryIsNotWrapped() {
        val argv = listOf("/system/bin/echo", "hello")
        val wrapped = terminal.emulator.runtime.wrapTermuxExec(argv, "/data/user/0/com.termux/files/usr")
        org.junit.Assert.assertEquals(argv, wrapped)
    }

    @org.junit.Test
    fun bareCommandIsNotWrapped() {
        val argv = listOf("echo", "hello")
        val wrapped = terminal.emulator.runtime.wrapTermuxExec(argv, "/data/user/0/com.termux/files/usr")
        org.junit.Assert.assertEquals(argv, wrapped)
    }

    @org.junit.Test
    fun nullPrefixIsNotWrapped() {
        val argv = listOf("/data/user/0/com.termux/files/usr/bin/echo")
        val wrapped = terminal.emulator.runtime.wrapTermuxExec(argv, null)
        org.junit.Assert.assertEquals(argv, wrapped)
    }
}
