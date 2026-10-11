package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PasteChunksTest {

    @Test
    fun blankTextTruncatesToEmpty() {
        assertEquals("", truncatePasteText(""))
        assertEquals("", truncatePasteText("   "))
    }

    @Test
    fun shortTextPassesThroughUntouched() {
        // 不解释内容：换行归一由上游编码承担，此处连换行也不碰。
        assertEquals("a\nb", truncatePasteText("a\nb"))
        assertEquals("hello", truncatePasteText("hello"))
    }

    @Test
    fun truncationCapsAtMaxChars() {
        assertEquals("0123456789", truncatePasteText("0123456789ABCDEF", maxChars = 10))
    }

    @Test
    fun truncationNeverSplitsSurrogatePair() {
        // maxChars 恰切在表情中间（高代理项在末尾）时多取一字符保住完整码点。
        assertEquals("ab😀", truncatePasteText("ab😀c", maxChars = 3))
    }

    @Test
    fun emptyStreamYieldsNoChunks() {
        assertTrue(pasteByteChunks(ByteArray(0)).isEmpty())
    }

    @Test
    fun shortStreamIsOneChunk() {
        assertEquals(listOf("hello"), pasteByteChunks("hello".toByteArray()).map { it.toString(Charsets.UTF_8) })
    }

    @Test
    fun longStreamSplitsOnByteBudget() {
        val chunks = pasteByteChunks("abcdefghij".toByteArray(), chunkBytes = 4)
        assertEquals(listOf("abcd", "efgh", "ij"), chunks.map { it.toString(Charsets.UTF_8) })
    }

    @Test
    fun splitNeverLandsInsideMultibyteCharacter() {
        // "a😀b" = 61 F0 9F 98 80 62：预算 4 若直切会把 emoji 劈成 F0 9F | 98 80。
        val chunks = pasteByteChunks("a😀b".toByteArray(), chunkBytes = 4)
        assertEquals(listOf("a", "😀", "b"), chunks.map { it.toString(Charsets.UTF_8) })
    }

    @Test
    fun reassemblyIsIdentity() {
        val original = "中文😀a\nb".toByteArray()
        val joined = pasteByteChunks(original, chunkBytes = 5).reduce { first, second -> first + second }
        assertEquals(original.toList(), joined.toList())
    }
}
