package terminal.emulator.runtime

import android.view.Choreographer
import java.nio.ByteBuffer
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * 把 PTY 写入按帧合并为一块，使输入法输入避开逐字节的原生调用路径。
 *
 * 原生 [flushSink] 写入永不阻塞：PTY 主 fd 是 O_NONBLOCK，
 * 故 PTY 缓冲满时（子进程未读取——前台应用挂起、超大粘贴）返回 EAGAIN 并丢弃字节，
 * 即 xterm 式背压丢失。下方单守护执行器把这些丢弃挡在输入法主线程
 * 与 Choreographer 帧回调之外（ANR 会被当作进程杀死）。
 * 直接调用方（经 viewModel.writeToPty 的 TerminalSurface 按键/软键盘路径）
 * 在其自身线程上有同样的 EAGAIN 丢弃行为。
 * 所有 sink 调用都跑在单个守护发送线程上：调用方永不阻塞，
 * 且单线程执行器在并发写入之间保持排空顺序。
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
    private val fallbackHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private val fallbackFlush = Runnable { flush() }
    private val sender: ExecutorService =
        Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "PtyWriter").apply { isDaemon = true }
        }

    fun write(data: ByteArray) {
        // 组合提交直发：单次 IME 组合提交量级（UTF-8 多码点短语，含 3 字节/码点
        // 汉字与 4 字节 emoji）的 write 跳过帧同步，延迟与退格直连路径对齐；
        // 中文一次 commitText 常含多个码点，编码后超单码点上限即被帧调度钳制，
        // 帧等待按次累加即体感退格慢。仅超过组合提交量级的（粘贴、大批量、
        // 编程性写入）仍走批缓冲合并写。保序：先排空驻留字节再直发，避免后写先到。
        if (data.size <= COMPOSITION_COMMIT_MAX_BYTES) {
            val pending = synchronized(lock) { drainLocked() }
            if (pending.isNotEmpty()) send(pending)
            if (data.isNotEmpty()) send(data)
            return
        }
        val toSend = ArrayList<ByteArray>(2)
        synchronized(lock) {
            if (data.size > capacity) {
                // 先 flush 已缓冲的输入，使先前排队的字节写在大块之前
                // ——否则顺序会颠倒（大块粘贴越过更早的击键）。
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
        fallbackHandler.removeCallbacks(fallbackFlush)
        val bytes = synchronized(lock) { drainLocked() }
        if (bytes.isNotEmpty()) send(bytes)
    }

    /** 排空缓冲区。必须在持有 [lock] 时调用。 */
    private fun drainLocked(): ByteArray {
        buffer.flip()
        val bytes = ByteArray(buffer.remaining())
        buffer.get(bytes)
        buffer.clear()
        scheduled = false
        return bytes
    }

    /** 把 [bytes] 交给单个发送线程处理（绝不阻塞调用方）。 */
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
            // 已调用 close()（视图已分离）；待处理输入被丢弃。
        }
    }

    /**
     * 停止发送线程。应在宿主视图的 detach 路径调用，
     * 否则重建的视图会泄漏一个「PtyWriter」线程（每个 TerminalSurface 实例一个）。
     */
    fun close() {
        // 在关闭前 flush 已缓冲的字节——否则 detach 前最后一帧输入的击键会被静默丢弃
        // （缓冲区只由帧回调或显式 flush 排空）。
        val pending = synchronized(lock) { drainLocked() }
        if (pending.isNotEmpty()) {
            try {
                sender.execute { flushSink(pending) }
            } catch (_: java.util.concurrent.RejectedExecutionException) {
                // shutdown 与入队竞争；在 detach 时丢弃可接受（视图已不存在）。
            }
        }
        sender.shutdown()
    }

    private fun scheduleFrame() {
        if (!useChoreographer) return
        // Choreographer 在应用不渲染任何内容时会静默丢弃帧回调
        // （空闲降频完全停止出帧），于是已缓冲的击键永远等不到，
        // 输入法提交在打字中途丢失。Handler 回退保证零帧时也能排空。
        scheduled = true
        fallbackHandler.removeCallbacks(fallbackFlush)
        fallbackHandler.postDelayed(fallbackFlush, FALLBACK_FLUSH_TIMEOUT_MS)
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
        private const val FALLBACK_FLUSH_TIMEOUT_MS = 50L

        /** 组合提交直发上限：UTF-8 汉字 3 字节/码点，64B 覆盖整句级 commitText
         *  （约 21 汉字）；粘贴/大批量/编程性写入通常数百字节以上，仍走批缓冲。 */
        private const val COMPOSITION_COMMIT_MAX_BYTES = 64

        /** 供测试用的工厂——避免依赖 Choreographer。 */
        fun forTest(flushSink: (ByteArray) -> Unit, capacity: Int = BATCH_CAPACITY): InputBatchBuffer =
            InputBatchBuffer(flushSink, capacity, useChoreographer = false)
    }
}
