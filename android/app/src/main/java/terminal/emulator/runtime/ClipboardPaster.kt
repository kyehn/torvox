package terminal.emulator.runtime

/** 两层调用共用的粘贴实现：经 [ClipboardAccess] 读取、[PasteChunker] 分块后交给 `sink`。 */
class ClipboardPaster(private val clipboard: ClipboardAccess, private val chunker: PasteChunker = PasteChunker()) {
    /**
     * 逐块粘贴当前剪贴板内容。
     * 返回入队到最后一个块边界的字符数（截断之后），采用 xterm 式「已接受」计数而非精确送达字节数。
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
