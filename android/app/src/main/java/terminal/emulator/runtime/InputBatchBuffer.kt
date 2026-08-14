package terminal.emulator.runtime

import android.view.Choreographer
import java.nio.ByteBuffer
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * Batches PTY writes to one chunk per frame, keeping IME input off the
 * per-byte native-call path.
 *
 * The native [flushSink] write never blocks: the PTY master fd is
 * O_NONBLOCK, so a full PTY buffer (child not reading — suspended
 * foreground app, huge paste) returns EAGAIN and the bytes are dropped,
 * xterm-style backpressure loss. The single daemon executor below keeps
 * those drops off the IME main thread and the Choreographer frame
 * callback (an ANR is treated as a process kill). Direct callers
 * (TerminalSurface key/soft-keyboard paths via viewModel.writeToPty)
 * have the same drop-on-EAGAIN behavior on the caller's thread.
 * All sink invocations run on a single daemon sender thread: the caller
 * never blocks, and the single-threaded executor preserves drain order
 * across concurrent writers.
 */
class InputBatchBuffer(
    private val flushSink: (ByteArray) -> Unit,
    private val capacity: Int = BATCH_CAPACITY,
    private val useChoreographer: Boolean = true,
) {
    private val lock = Any()
    private var buffer: ByteBuffer = ByteBuffer.allocateDirect(capacity)
    private var frameCallback: Choreographer.FrameCallback? = null
    private var scheduled = false
    private val sender: ExecutorService =
        Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "PtyWriter").apply { isDaemon = true }
        }

    fun write(data: ByteArray) {
        val toSend = ArrayList<ByteArray>(2)
        synchronized(lock) {
            if (data.size > capacity) {
                // Flush buffered input first so previously queued bytes
                // are written before the large chunk — otherwise ordering
                // is inverted (big paste overtakes earlier keystrokes).
                toSend.add(drainLocked())
                toSend.add(data)
            } else {
                if (buffer.remaining() < data.size) {
                    toSend.add(drainLocked())
                }
                buffer.put(data)
                if (!scheduled) {
                    scheduleFrame()
                }
            }
        }
        for (chunk in toSend) {
            if (chunk.isNotEmpty()) send(chunk)
        }
    }

    fun flush() {
        val bytes = synchronized(lock) { drainLocked() }
        if (bytes.isNotEmpty()) send(bytes)
    }

    /** Drains the buffer. Must be called while holding [lock]. */
    private fun drainLocked(): ByteArray {
        buffer.flip()
        val bytes = ByteArray(buffer.remaining())
        buffer.get(bytes)
        buffer.clear()
        scheduled = false
        return bytes
    }

    /** Handles [bytes] to the single sender thread (never blocks the caller). */
    private fun send(bytes: ByteArray) {
        try {
            sender.execute {
                try {
                    flushSink(bytes)
                } catch (exception: Exception) {
                    LogUtil.e("InputBatchBuffer", "PTY write failed", exception)
                }
            }
        } catch (exception: java.util.concurrent.RejectedExecutionException) {
            // close() was called (view detached); pending input is dropped.
        }
    }

    /**
     * Stops the sender thread. Call from the owning view's detach path so a
     * recreated view does not leak a "PtyWriter" thread (one per
     * TerminalSurface instance).
     */
    fun close() {
        // flush buffered bytes BEFORE shutting down —
        // otherwise keystrokes typed in the final frame before detach are
        // silently dropped (the buffer is drained only by the frame
        // callback or an explicit flush).
        val pending = synchronized(lock) { drainLocked() }
        if (pending.isNotEmpty()) {
            try {
                sender.execute { flushSink(pending) }
            } catch (_: java.util.concurrent.RejectedExecutionException) {
                // Shutdown raced the enqueue; dropping is acceptable at
                // detach (the view is gone).
            }
        }
        sender.shutdown()
    }

    private fun scheduleFrame() {
        if (!useChoreographer) return
        scheduled = true
        if (frameCallback == null) {
            frameCallback = Choreographer.FrameCallback { _ -> flush() }
        }
        Choreographer.getInstance().postFrameCallback(
            frameCallback
                ?: error("frameCallback must be initialized before use"),
        )
    }

    fun reset() {
        synchronized(lock) {
            buffer.clear()
            scheduled = false
        }
    }

    companion object {
        private const val BATCH_CAPACITY = 8192

        /** Factory for test usage — avoids Choreographer dependency. */
        fun forTest(
            flushSink: (ByteArray) -> Unit,
            capacity: Int = BATCH_CAPACITY,
        ): InputBatchBuffer = InputBatchBuffer(flushSink, capacity, useChoreographer = false)
    }
}
