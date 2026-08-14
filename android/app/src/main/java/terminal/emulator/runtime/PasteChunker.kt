package terminal.emulator.runtime

import android.util.Log

/**
 * Chunks clipboard text for PTY paste.
 *
 * One synchronous `feedPty` call with a full multi-megabyte payload always
 * exceeds the PTY kernel buffer (~64KB) and is dropped wholesale on EAGAIN.
 * Chunking through the per-frame flush path gives the child shell time to
 * drain between chunks. Chunks are split on code-point boundaries (never
 * inside a surrogate pair).
 *
 * This is the single implementation used by both the view layer
 * ([terminal.emulator.ui.TerminalSurface]) and the view-model layer
 * ([terminal.emulator.TerminalViewModel]) — previously the identical
 * constants + loop existed twice.
 */
class PasteChunker(
    private val maxChars: Int = MAX_PASTE_CHARS,
    private val chunkChars: Int = CHUNK_CHARS,
    private val tag: String = "PasteChunker",
) {
    /**
     * Normalize and split [text] into chunks ready for PTY writes.
     *
     * Returns an empty list when [text] is blank. Truncates to [maxChars]
     * (with a warning) and translates `\n` to `\r` (xterm paste semantics).
     */
    fun chunks(text: String): List<String> {
        if (text.isBlank()) return emptyList()
        if (text.length > maxChars) {
            Log.w(
                tag,
                "clipboard too large (${text.length} chars), truncating to $maxChars",
            )
        }
        val normalized = text.take(maxChars).replace("\n", "\r")
        val chunks = mutableListOf<String>()
        var offset = 0
        while (offset < normalized.length) {
            var end = minOf(offset + chunkChars, normalized.length)
            if (end < normalized.length && Character.isHighSurrogate(normalized[end - 1])) {
                end -= 1
            }
            if (end <= offset) break
            chunks.add(normalized.substring(offset, end))
            offset = end
        }
        return chunks
    }

    companion object {
        /** Upper bound: keeps string copies (toString/replace/toByteArray)
         *  from OOMing the caller thread on a huge clipboard. */
        const val MAX_PASTE_CHARS = 1_000_000

        /** Must stay well below the PTY kernel buffer (~64KB). */
        const val CHUNK_CHARS = 4_000
    }
}
