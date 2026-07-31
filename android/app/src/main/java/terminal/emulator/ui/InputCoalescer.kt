package terminal.emulator.ui

import java.util.concurrent.atomic.AtomicReference

/**
 * Tracks IME composing state and forwards committed text to the PTY.
 *
 * The previous implementation attempted to deduplicate IME double-fires
 * (Gboard/Samsung [android.view.inputmethod.BaseInputConnection.commitText]
 * bugs) by buffering single bytes and collapsing pairs of identical bytes.
 * That logic never fired: [send] flushed synchronously, so the buffer held
 * exactly one byte at every flush, and a frame-delayed flush would have been
 * worse — it cannot distinguish a genuine fast "aa" keystroke from a
 * double-fire, silently eating real input. Dedup was therefore removed and
 * [send] forwards verbatim; input correctness wins over a speculative IME bug.
 *
 * @param sink receives the bytes to forward to the PTY.
 */
class InputCoalescer(
    private val sink: (ByteArray) -> Unit,
) {
    private val composingText = AtomicReference<String?>(null)

    fun send(data: ByteArray) {
        sink(data)
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
        composingText.set(null)
    }
}
