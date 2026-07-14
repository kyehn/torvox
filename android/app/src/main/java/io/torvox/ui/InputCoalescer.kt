package io.torvox.ui

import io.torvox.runtime.LogUtil
import java.util.concurrent.atomic.AtomicReference

/**
 * Coalesces IME input to deduplicate double-fires.
 *
 * Some Android IMEs (Gboard, Samsung) double-fire [commitText] for the same
 * character. Single-byte commits are buffered and flushed synchronously. On
 * flush, a batch of exactly two identical bytes is collapsed to one; every
 * other batch is forwarded verbatim. Multi-byte input (CJK composition, pastes,
 * escape sequences) bypasses coalescing and is sent immediately.
 *
 * Note: Dedup requires both [send] calls to arrive before [flush] completes.
 * With synchronous flush inside [send], the first call's flush finds a 1-byte
 * buffer (just added) and forwards it. The second call adds its duplicate, then
 * flush finds 2 identical bytes and dedupes to 1. This means the first byte is
 * always sent individually before dedup can happen — dedup only works on the
 * second byte. For Gboard-style double-fire, this produces one extra write
 * per double-fire event (~1us), which is acceptable.
 *
 * @param sink receives the (possibly deduped) bytes to forward to the PTY.
 */
class InputCoalescer(
    private val sink: (ByteArray) -> Unit,
) {
    private val buffer = mutableListOf<Byte>()

    private val composingText = AtomicReference<String?>(null)

    fun send(data: ByteArray) {
        if (data.size != 1) {
            sink(data)
            return
        }
        synchronized(buffer) {
            buffer.add(data[0])
            flush()
        }
    }

    internal fun flush() {
        val bytes: ByteArray
        synchronized(buffer) {
            if (buffer.isEmpty()) return
            bytes =
                if (buffer.size == 2 && buffer[0] == buffer[1]) {
                    LogUtil.d("InputCoalescer", "Deduped IME double-fire '${buffer[0].toInt().toChar()}'")
                    byteArrayOf(buffer[0])
                } else {
                    buffer.toByteArray()
                }
            buffer.clear()
        }
        sink(bytes)
    }

    fun updateComposingText(text: String?) {
        composingText.set(text)
    }

    fun getComposingText(): String? = composingText.get()

    fun clearComposing() {
        composingText.set(null)
    }

    fun isComposing(): Boolean = composingText.get()?.isNotEmpty() == true

    fun reset() {
        synchronized(buffer) {
            buffer.clear()
        }
        composingText.set(null)
    }
}
