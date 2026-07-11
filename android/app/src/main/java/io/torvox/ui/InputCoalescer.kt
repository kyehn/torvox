package io.torvox.ui

import android.os.Handler
import android.os.Looper
import io.torvox.runtime.LogUtil
import java.util.concurrent.atomic.AtomicReference

/**
 * Deduplicates IME input double-fires using Haven's message-batch signature.
 *
 * Some Android IMEs (Gboard, Samsung) double-fire [commitText] for the same
 * character within a single input message. Genuine IME double-fires arrive as a
 * batch of exactly two identical bytes; legitimate fast typing or a paste
 * arrives as longer / multi-byte input that must be preserved.
 *
 * Single-byte commits are buffered and flushed on the next looper turn (after
 * the current message completes). On flush, a batch that is exactly two
 * identical bytes is collapsed to one byte; every other batch is forwarded
 * verbatim. Multi-byte input (CJK composition, pastes, escape sequences) bypasses
 * coalescing and is sent immediately, so it is never dropped.
 *
 * Modeled on Haven's `TerminalViewModel.InputCoalescer`
 * (`feature/terminal/.../TerminalViewModel.kt:185-225`).
 *
 * @param sink receives the (possibly deduped) bytes to forward to the PTY.
 * @param scheduler posts the flush runnable; defaults to the main [Handler] so
 *   the batch flushes after the current input message. Tests may inject a
 *   capturing scheduler and call [flush] manually to avoid a live looper.
 */
class InputCoalescer(
    private val sink: (ByteArray) -> Unit,
    private val scheduler: (Runnable) -> Unit = {
        Handler(Looper.getMainLooper()).post(it)
    },
) {
    private val buffer = mutableListOf<Byte>()

    private val composingText = AtomicReference<String?>(null)

    private val flushRunnable =
        Runnable {
            flush()
        }

    fun send(data: ByteArray) {
        if (data.size != 1) {
            // Multi-byte input (CJK, pastes, escape sequences) — flush any pending
            // single-byte batch first, then send directly without dedup.
            flush()
            sink(data)
            return
        }
        synchronized(buffer) {
            buffer.add(data[0])
        }
        // Post flush to run after the current message completes. Both IME
        // double-fire calls and a paste's byte iteration happen within one
        // message, so the flush observes the complete batch.
        scheduler(flushRunnable)
    }

    internal fun flush() {
        val bytes: ByteArray
        synchronized(buffer) {
            if (buffer.isEmpty()) return
            // IME double-fire signature: exactly two identical bytes.
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
