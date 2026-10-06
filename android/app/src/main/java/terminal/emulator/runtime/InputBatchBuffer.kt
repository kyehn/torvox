package terminal.emulator.runtime

import android.view.Choreographer
import java.nio.ByteBuffer
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * 把 PTY 写入按帧合并为一块，使输入法输入避开逐字节的原生调用路径。
 *
 * 原生 [flushSink] 写入**可能阻塞**：PTY 主 fd 虽是 O_NONBLOCK，但原生
 * `write_all` 在 WouldBlock 上 poll 到子进程消费（上限见 native 的
 * `WRITE_DRAIN_TIMEOUT`，5s），绝不丢弃字节——丢弃会让粘贴被静默截断成半条命令。
 * 下方单守护执行器正是为把这段等待挡在输入法主线程与 Choreographer 帧回调之外
 * （ANR 会被当作进程杀死）。
 * 直接调用方（经 viewModel.writeToPty 的 TerminalSurface 按键/软键盘路径）
 * 在其自身线程上承受同一等待。
 * 所有 sink 调用都跑在单个守护发送线程上：调用方永不阻塞，
 * 且单线程执行器在并发写入之间保持排空顺序。代价是该线程被所有会话共享，
 * 一个不读 stdin 的子进程会把其余会话的输入一并堵住。
 *
 * 每次 [write] 都在调用点捕获目标会话（[inputSessionId]），随字节一起送往下沉：
 * 刷写发生在别处（帧回调或 PtyWriter 线程），到那时再解析活动会话会把
 * 粘贴尾部写进用户刚切换到的新会话。会话变化时先按旧会话排空，保证顺序。
 */
class InputBatchBuffer(
    private val flushSink: (Long, ByteArray) -> Unit,
    private val inputSessionId: () -> Long,
    private val capacity: Int = BATCH_CAPACITY,
    private val useChoreographer: Boolean = true,
) {
    private val lock = Any()
    private var buffer: ByteBuffer = ByteBuffer.allocateDirect(capacity)
    private var frameCallback: Choreographer.FrameCallback? = null
    private var scheduled = false
    private var bufferedSessionId = 0L
    private val fallbackHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private val fallbackFlush = Runnable { flush() }
    private val sender: ExecutorService =
        Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "PtyWriter").apply { isDaemon = true }
        }

    fun write(data: ByteArray, sessionId: Long = inputSessionId()) {
        // 组合提交直发：单次 IME 组合提交量级（UTF-8 多码点短语，含 3 字节/码点
        // 汉字与 4 字节 emoji）的 write 跳过帧同步，延迟与退格直连路径对齐；
        // 中文一次 commitText 常含多个码点，编码后超单码点上限即被帧调度钳制，
        // 帧等待按次累加即体感退格慢。仅超过组合提交量级的（粘贴、大批量、
        // 编程性写入）仍走批缓冲合并写。保序：先排空驻留字节再直发，避免后写先到。
        if (data.size <= COMPOSITION_COMMIT_MAX_BYTES) {
            synchronized(lock) {
                val (pendingId, pending) = drainLocked()
                sendLocked(pendingId, pending)
                sendLocked(sessionId, data)
            }
            return
        }
        synchronized(lock) {
            if (data.size > capacity) {
                // 先 flush 已缓冲的输入，使先前排队的字节写在大块之前
                // ——否则顺序会颠倒（大块粘贴越过更早的击键）。
                val (pendingId, pending) = drainLocked()
                sendLocked(pendingId, pending)
                sendLocked(sessionId, data)
            } else {
                if (buffer.position() != 0 && bufferedSessionId != sessionId) {
                    // 目标会话已变：旧会话的字节必须先走，绝不并入新会话。
                    val (staleId, stale) = drainLocked()
                    sendLocked(staleId, stale)
                }
                if (buffer.remaining() < data.size) {
                    val (fullId, full) = drainLocked()
                    sendLocked(fullId, full)
                }
                bufferedSessionId = sessionId
                buffer.put(data)
                if (!scheduled) {
                    scheduleFrame()
                }
            }
        }
    }

    fun flush() {
        fallbackHandler.removeCallbacks(fallbackFlush)
        synchronized(lock) {
            val (pendingId, pending) = drainLocked()
            sendLocked(pendingId, pending)
        }
    }

    /** 排空缓冲区，连同驻留字节的目标会话。必须在持有 [lock] 时调用。 */
    private fun drainLocked(): Pair<Long, ByteArray> {
        buffer.flip()
        val bytes = ByteArray(buffer.remaining())
        buffer.get(bytes)
        buffer.clear()
        scheduled = false
        return bufferedSessionId to bytes
    }

    /**
     * 把 [bytes] 交给单个发送线程处理（绝不阻塞调用方：执行器队列无界，
     * 入队永不阻塞，故可在持有 [lock] 时调用）。
     *
     * 必须在持有 [lock] 时调用：排空与入队原子，并发写入者不再可能
     * 按 X、Y 排空却按 Y、X 入队。
     */
    private fun sendLocked(sessionId: Long, bytes: ByteArray) {
        if (bytes.isEmpty()) return
        try {
            sender.execute {
                try {
                    flushSink(sessionId, bytes)
                } catch (exception: Exception) {
                    LogUtil.e("InputBatchBuffer", "PTY write failed", exception)
                }
            }
        } catch (exception: java.util.concurrent.RejectedExecutionException) {
            // 已调用 close()（视图已分离）；待处理输入被丢弃。必须记日志：
            // 静默丢弃会让输入与粘贴在没有任何症状的情况下彻底失效。
            LogUtil.w("InputBatchBuffer", "PTY write rejected: batch buffer closed", exception)
        }
    }

    /**
     * 停止发送线程。应在宿主视图的 detach 路径调用，
     * 否则重建的视图会泄漏一个「PtyWriter」线程（每个 TerminalSurface 实例一个）。
     */
    fun close() {
        // 在关闭前 flush 已缓冲的字节——否则 detach 前最后一帧输入的击键会被静默丢弃
        // （缓冲区只由帧回调或显式 flush 排空）。
        synchronized(lock) {
            val (pendingId, pending) = drainLocked()
            if (pending.isNotEmpty()) {
                try {
                    sender.execute { flushSink(pendingId, pending) }
                } catch (exception: java.util.concurrent.RejectedExecutionException) {
                    // shutdown 与入队竞争；在 detach 时丢弃可接受（视图已不存在）。
                    LogUtil.w("InputBatchBuffer", "final flush rejected during close", exception)
                }
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

    companion object {
        private const val BATCH_CAPACITY = 8192
        private const val FALLBACK_FLUSH_TIMEOUT_MS = 50L

        /** 组合提交直发上限：UTF-8 汉字 3 字节/码点，64B 覆盖整句级 commitText
         *  （约 21 汉字）；粘贴/大批量/编程性写入通常数百字节以上，仍走批缓冲。 */
        private const val COMPOSITION_COMMIT_MAX_BYTES = 64

        /** 供测试用的工厂——避免依赖 Choreographer。 */
        fun forTest(
            flushSink: (Long, ByteArray) -> Unit,
            capacity: Int = BATCH_CAPACITY,
            inputSessionId: () -> Long = { 1L },
        ): InputBatchBuffer = InputBatchBuffer(flushSink, inputSessionId, capacity, useChoreographer = false)
    }
}
