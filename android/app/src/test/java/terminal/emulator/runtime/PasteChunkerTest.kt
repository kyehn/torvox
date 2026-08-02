package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PasteChunkerTest {
    private val chunker = PasteChunker(maxChars = 1_000_000, chunkChars = 4_000)

    @Test
    fun blankTextYieldsNoChunks() {
        assertTrue(chunker.chunks("").isEmpty())
        assertTrue(chunker.chunks("   ").isEmpty())
    }

    @Test
    fun shortTextIsOneChunk() {
        assertEquals(listOf("hello"), chunker.chunks("hello"))
    }

    @Test
    fun newlineTranslatedToCarriageReturn() {
        assertEquals(listOf("a\rb"), chunker.chunks("a\nb"))
    }

    @Test
    fun longTextSplitOnChunkBoundary() {
        val chunker4 = PasteChunker(maxChars = 1_000_000, chunkChars = 4)
        val chunks = chunker4.chunks("abcdefghij")
        assertEquals(listOf("abcd", "efgh", "ij"), chunks)
    }

    @Test
    fun surrogatePairNeverSplit() {
        // "😀" is a surrogate pair (2 chars); chunk of 3 must not split it.
        val chunker3 = PasteChunker(maxChars = 1_000_000, chunkChars = 3)
        val text = "a😀b"
        val chunks = chunker3.chunks(text)
        // "a" + "😀" would be 3 chars but the pair must stay together,
        // so chunks are ["a😀", "b"].
        assertEquals(listOf("a😀", "b"), chunks)
    }

    @Test
    fun truncationCapsAtMaxChars() {
        val capped = PasteChunker(maxChars = 10, chunkChars = 4)
        val chunks = capped.chunks("0123456789ABCDEF")
        assertEquals("0123456789", chunks.joinToString(""))
    }

    @Test
    fun emojiAtExactChunkEndSurvives() {
        val chunker4 = PasteChunker(maxChars = 1_000_000, chunkChars = 4)
        // "😀" occupies positions 1-2; chunk of 4 with text "a😀b" -> whole string is 4 chars.
        val chunks = chunker4.chunks("a😀b")
        assertEquals(listOf("a😀b"), chunks)
    }
}
