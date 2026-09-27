package terminal.emulator.monitor

import android.os.Handler
import android.os.Looper
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

class AnrWatchDog(
    private val stateDir: File,
    private val timeoutMs: Long = ANR_TIMEOUT_MILLIS,
    private val warmUpMillis: Long = WARM_UP_MILLIS,
    private val onAnr: () -> Unit = { BootGuard.exit(stateDir, "ANR") },
) {
    private val mainHandler = Handler(Looper.getMainLooper())

    private val running = AtomicBoolean(false)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var watchJob: Job? = null
    private val anrInProgress = AtomicBoolean(false)
    private val completed = AtomicBoolean(false)

    // 每次 start() 递增；代次不再匹配的看门狗线程自行退出
    // ——使 stop()→start() 循环中存活过 join 超时的旧线程不会留下两个监视者。
    private val generation = AtomicInteger(0)

    fun start() {
        // CAS：两个并发调用方绝不能启动两个看门狗线程（每个都可能杀掉进程）。
        if (!running.compareAndSet(false, true)) return
        val myGeneration = generation.incrementAndGet()
        completed.set(false)
        anrInProgress.set(false)
        watchJob =
            scope.launch {
                watchLoop(myGeneration)
            }
    }

    // 防御性 API：当前无生产调用方（看门狗存活于整个进程生命周期）。
    // 保留代次握手，使未来的调用方不会泄漏陈旧的监视者。
    fun stop() {
        running.set(false)
        generation.incrementAndGet()
        val job = watchJob
        watchJob = null
        if (job != null) {
            runBlocking {
                withTimeoutOrNull(1000L) { job.cancelAndJoin() }
            }
        }
    }

    private suspend fun watchLoop(myGeneration: Int) {
        // 预热窗口：冷启动（Hilt 注入、首个 Compose 帧、DataStore 读取）
        // 在慢速设备上经常超过 5s；一次误报就会杀掉进程并丢失所有会话。
        // 在应用已运行一段时间之前跳过检查。
        val startUpNanos = System.nanoTime()
        while (System.nanoTime() - startUpNanos < warmUpMillis * 1_000_000L) {
            if (!running.get() || generation.get() != myGeneration) return
            delay(WARM_UP_SLEEP_MILLIS)
        }
        while (running.get() && generation.get() == myGeneration) {
            if (anrInProgress.get()) {
                delay(timeoutMs)
                continue
            }
            completed.set(false)
            mainHandler.post {
                completed.set(true)
            }
            val startMs = System.currentTimeMillis()
            while (running.get() && generation.get() == myGeneration) {
                val elapsed = System.currentTimeMillis() - startMs
                if (elapsed >= timeoutMs) {
                    onAnrDetected()
                    // ANR 是终止性的：要么 BootGuard 杀掉进程，
                    // 要么杀进程被抑制且 dump 已记录。在此重新武装
                    // 会每 `timeoutMs` 再次触发（在被抑制的情形下填满数据分区）
                    // ——改为停止监视者。
                    return
                }
                if (completed.get()) break
                delay(BUSY_WAIT_SLEEP_MILLIS)
            }
        }
    }

    private fun onAnrDetected() {
        if (!anrInProgress.compareAndSet(false, true)) return
        try {
            val suppressed = !BootGuard.autoKillEnabled
            if (suppressed) {
                // BootGuard 已在反复退出后抑制了杀进程。
                // 主线程持续阻塞时每 5s 写一份完整线程 dump 会填满数据分区
                // （dump 会 fsync 且从不轮转），故此状态下只写 logcat。
                Log.e("AnrWatchDog", "ANR suppressed by BootGuard; skipping dump")
                return
            }
            val stackTraces = StringBuilder()
            val mainStackTrace = Looper.getMainLooper().thread.stackTrace
            stackTraces.appendLine("== ANR Detected ==")
            stackTraces.appendLine("Timeout: ${timeoutMs}ms")
            stackTraces.appendLine()
            stackTraces.appendLine("--- Main Thread ---")
            for (element in mainStackTrace) {
                stackTraces.appendLine("\tat $element")
            }
            stackTraces.appendLine()
            stackTraces.appendLine("--- All Threads ---")
            val threadStacks = Thread.getAllStackTraces()
            for ((thread, trace) in threadStacks) {
                if (thread == Looper.getMainLooper().thread) continue
                stackTraces.appendLine("${thread.name} (priority=${thread.priority}, state=${thread.state})")
                for (element in trace) {
                    stackTraces.appendLine("\tat $element")
                }
                stackTraces.appendLine()
            }

            Log.e("AnrWatchDog", "ANR detected, killing process:\n$stackTraces")
            onAnr()
        } catch (e: Exception) {
            Log.e("AnrWatchDog", "Unhandled exception in ANR handler", e)
        } finally {
            anrInProgress.set(false)
        }
    }

    companion object {
        private const val ANR_TIMEOUT_MILLIS = 5_000L
        private const val BUSY_WAIT_SLEEP_MILLIS = 100L
        private const val WARM_UP_MILLIS = 20_000L
        private const val WARM_UP_SLEEP_MILLIS = 500L
    }
}
