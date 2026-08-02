package terminal.emulator.runtime

/**
 * One paste implementation for both call layers (K2: round-2 architecture).
 *
 * Previously TerminalViewModel.pasteFromClipboard and
 * TerminalSurface.pasteFromClipboardDirect each had their own
 * clipboard-read + PasteChunker loop with near-identical comments claiming
 * "shared chunking". This class is the single source: read clipboard via
 * [ClipboardAccess], chunk via [PasteChunker], hand each chunk to the
 * caller-supplied sink.
 */
class ClipboardPaster(
    private val clipboard: ClipboardAccess,
    private val chunker: PasteChunker = PasteChunker(),
) {
    /**
     * Paste the current clipboard through [sink] (one call per chunk).
     *
     * Returns the number of characters queued up to the last successful
     * chunk boundary (post-truncation); a chunk dropped by PTY backpressure
     * (EAGAIN) is still counted — the xterm-style "accepted" count, not
     * byte-exact delivery (round-112 semantics).
     */
    fun pasteTo(sink: (ByteArray) -> Unit): Int {
        val text = clipboard.clipboardText() ?: return 0
        var offset = 0
        for (chunk in chunker.chunks(text)) {
            sink(chunk.toByteArray())
            offset += chunk.length
        }
        return offset
    }
}
