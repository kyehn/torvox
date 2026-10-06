package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PasteChunksTest {

    @Test
    fun blankTextYieldsNoChunks() {
        assertTrue(pasteChunks("").isEmpty())
        assertTrue(pasteChunks("   ").isEmpty())
    }

    @Test
    fun shortTextIsOneChunk() {
        assertEquals(listOf("hello"), pasteChunks("hello"))
    }

    @Test
    fun newlineTranslatedToCarriageReturn() {
        assertEquals(listOf("a\rb"), pasteChunks("a\nb"))
    }

    @Test
    fun longTextSplitOnChunkBoundary() {
        assertEquals(listOf("abcd", "efgh", "ij"), pasteChunks("abcdefghij", chunkChars = 4))
    }

    @Test
    fun surrogatePairNeverSplit() {
        // "😀" 是代理对（2 个 char）；块大小为 3 时不得把它劈开。
        assertEquals(listOf("a😀", "b"), pasteChunks("a😀b", chunkChars = 3))
    }

    @Test
    fun truncationCapsAtMaxChars() {
        val chunks = pasteChunks("0123456789ABCDEF", maxChars = 10, chunkChars = 4)
        assertEquals("0123456789", chunks.joinToString(""))
    }

    @Test
    fun emojiAtExactChunkEndSurvives() {
        // "😀" 占 1-2 位；块大小 4 配 "a😀b" 恰为 4 个 char，不应拆分。
        assertEquals(listOf("a😀b"), pasteChunks("a😀b", chunkChars = 4))
    }

    @Test
    fun crlfCollapsesToSingleCarriageReturn() {
        // 回车换行必须只产生一次回车，否则每粘贴一行多出一个空行。
        assertEquals(listOf("a\rb"), pasteChunks("a\r\nb"))
    }

    @Test
    fun truncationNeverSplitsSurrogatePair() {
        // maxChars 恰切在表情中间（高代理项在末尾）时多取一字符保住完整码点。
        val chunks = pasteChunks("ab😀c", maxChars = 3, chunkChars = 4)
        assertEquals("ab😀", chunks.joinToString(""))
    }
}
