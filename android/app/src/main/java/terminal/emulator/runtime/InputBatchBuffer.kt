package terminal.emulator.runtime

import android.view.Choreographer
import java.nio.ByteBuffer

class InputBatchBuffer(
    private val flushSink: (ByteArray) -> Unit,
    private val capacity: Int = BATCH_CAPACITY,
    private val useChoreographer: Boolean = true,
) {
    private val lock = Any()
    private var buffer: ByteBuffer = ByteBuffer.allocateDirect(capacity)
    private var frameCallback: Choreographer.FrameCallback? = null
    private var scheduled = false

    fun write(data: ByteArray) {
        synchronized(lock) {
            if (data.size > capacity) {
                flushSink(data)
                return
            }
            if (buffer.remaining() < data.size) {
                flushInternal()
            }
            buffer.put(data)
            if (!scheduled) {
                scheduleFrame()
            }
        }
    }

    fun flush() {
        synchronized(lock) { flushInternal() }
    }

    private fun flushInternal() {
        buffer.flip()
        val bytes = ByteArray(buffer.remaining())
        buffer.get(bytes)
        buffer.clear()
        scheduled = false
        if (bytes.isNotEmpty()) {
            flushSink(bytes)
        }
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
        buffer.clear()
        scheduled = false
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
