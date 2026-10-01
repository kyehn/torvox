package terminal.emulator.runtime

/**
 * 为 PTY 粘贴切分剪贴板文本。
 *
 * 一次同步 `feedPty` 调用携带完整的多兆字节负载必然超出 PTY 内核缓冲（~64KB），
 * 并在 EAGAIN 时被整块丢弃。经逐帧 flush 路径切分可给子 shell 块间排空的时间。
 * 切分点落在码点边界（绝不在代理对内部）。
 */
class PasteChunker(
    private val maxChars: Int = MAX_PASTE_CHARS,
    private val chunkChars: Int = CHUNK_CHARS,
    private val tag: String = "PasteChunker",
) {
    /**
     * 归一化 [text] 并切分为可直接写入 PTY 的块。
     *
     * [text] 为空白时返回空列表。截断到 [maxChars]（并告警；截断点落在代理对
     * 中间时多取一字符保住完整码点），并把换行转成 `\r`（xterm 粘贴语义；
     * 先折叠 `\r\n`，否则回车换行会被展开成两个回车）。
     */
    fun chunks(text: String): List<String> {
        if (text.isBlank()) return emptyList()
        if (text.length > maxChars) {
            LogUtil.w(
                tag,
                "clipboard too large (${text.length} chars), truncating to $maxChars",
            )
        }
        var truncated = text.take(maxChars)
        if (text.length > maxChars && truncated.isNotEmpty() &&
            Character.isHighSurrogate(truncated.last()) &&
            Character.isLowSurrogate(text[maxChars])
        ) {
            truncated += text[maxChars]
        }
        val normalized = truncated.replace("\r\n", "\r").replace("\n", "\r")
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
        /** 上界：避免在超大剪贴板上让字符串拷贝（toString/replace/toByteArray）把调用线程 OOM。 */
        const val MAX_PASTE_CHARS = 1_000_000

        /** 必须远低于 PTY 内核缓冲（~64KB）。 */
        const val CHUNK_CHARS = 4_000
    }
}
