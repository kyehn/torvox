package terminal.emulator.runtime

/** 上界：避免在超大剪贴板上让字符串拷贝（toString/toByteArray）把调用线程 OOM。 */
const val MAX_PASTE_CHARS = 1_000_000

/** 编码流单块字节上界：必须远低于 PTY 内核缓冲（~64KB）。 */
const val PASTE_CHUNK_BYTES = 12_000

/** 截断告警的统一标签：粘贴、复制与分享三条路径共用。 */
private const val TRUNCATION_TAG = "TextTruncation"

/**
 * 按 [maxChars] 截断文本，截断点落在代理对中间时多取一字符保住完整码点。
 *
 * 只做截断，不解释内容：空白文本原样返回（空白选区是合法的复制/分享内容）。
 * 截断是降级动作而非正常流程，故一律记警告，不静默丢字。
 */
internal fun truncateTextAtCharBoundary(text: String, maxChars: Int): String {
    if (text.length <= maxChars) return text
    LogUtil.warn(TRUNCATION_TAG, "text too large (${text.length} chars), truncating to $maxChars")
    val truncated = text.take(maxChars)
    if (truncated.isNotEmpty() &&
        Character.isHighSurrogate(truncated.last()) &&
        Character.isLowSurrogate(text[maxChars])
    ) {
        return truncated + text[maxChars]
    }
    return truncated
}

/**
 * 截断剪贴板文本到 [maxChars]。空白文本返回空串，由调用方跳过。
 *
 * 只做截断，不解释内容：换行归一与括号包裹是上游 `paste::encode` 的协议语义，
 * 由编码后的字节流承担，本处不得提前改写原文（两次不同模式的粘贴必须
 * 一致地来自同一原文）。
 *
 * [maxChars] 只供单测缩小规模；生产只用默认值。
 */
internal fun truncatePasteText(text: String, maxChars: Int = MAX_PASTE_CHARS): String {
    if (text.isBlank()) return ""
    return truncateTextAtCharBoundary(text, maxChars)
}

/**
 * 把已编码的粘贴字节流切分为可直接写入 PTY 的块。
 *
 * 只切分不解释：上游编码产出的括号标记位于流首尾，中间切分天然不拆标记；
 * 切分点落在 UTF-8 字符边界（绝不在多字节字符内部）。
 */
internal fun pasteByteChunks(stream: ByteArray, chunkBytes: Int = PASTE_CHUNK_BYTES): List<ByteArray> {
    if (stream.isEmpty()) return emptyList()
    val chunks = mutableListOf<ByteArray>()
    var offset = 0
    while (offset < stream.size) {
        var end = minOf(offset + chunkBytes, stream.size)
        if (end < stream.size) {
            // 回退到字符边界：尾部连续字节（10xxxxxx）属于被切半的字符，
            // 连同其起始字节一并留给下一块。
            while (end > offset && isUtf8Continuation(stream[end - 1])) end--
            if (end > offset && isUtf8Lead(stream[end - 1])) end--
            if (end <= offset) {
                // 块小于一个完整字符（调用方传的 chunkBytes 过小）：向前吞完整字符。
                var next = offset + 1
                while (next < stream.size && isUtf8Continuation(stream[next])) next++
                end = next
            }
        }
        chunks.add(stream.copyOfRange(offset, end))
        offset = end
    }
    return chunks
}

private fun isUtf8Continuation(byte: Byte): Boolean = byte.toInt() and 0xC0 == 0x80

private fun isUtf8Lead(byte: Byte): Boolean = byte.toInt() and 0xC0 == 0xC0
