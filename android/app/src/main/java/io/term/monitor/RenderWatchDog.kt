package io.term.monitor

import android.util.Log

class RenderWatchDog(
    private val getStart: () -> Long,
    private val getDone: () -> Long,
    private val isRunning: () -> Boolean,
    private val onHangDetected: () -> Unit,
    private val hangTimeoutNanos: Long = 10_000_000_000L,
) {
    companion object {
        private const val CHECK_INTERVAL_MS = 2000L
        private const val TAG = "RenderWatchDog"
    }

    private val checker = Thread({ watchLoop() }, "RenderWatchDog").apply { isDaemon = true }

    fun start() {
        checker.start()
    }

    fun stop() {
        checker.interrupt()
    }

    private fun watchLoop() {
        while (!Thread.currentThread().isInterrupted) {
            try {
                Thread.sleep(CHECK_INTERVAL_MS)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
                break
            }
            val start = getStart()
            val done = getDone()
            val elapsed = System.nanoTime() - start
            if (start > done && elapsed > hangTimeoutNanos && isRunning()) {
                Log.e(TAG, "Render hang detected: elapsed=${elapsed / 1_000_000L}ms")
                onHangDetected()
            }
        }
    }
}
