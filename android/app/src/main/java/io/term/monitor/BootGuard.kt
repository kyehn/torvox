package io.term.monitor

import android.util.Log
import java.io.File

class BootGuard(
    private val logDir: File,
) {
    fun check() {
        synchronized(LOCK) {
            val counter = readCounter()
            val now = System.currentTimeMillis()

            if (counter.count >= MAX_EXITS && (now - counter.lastResetTime) < RESET_WINDOW_MS) {
                autoKillEnabled = false
                Log.w(
                    TAG,
                    "Boot loop detected: ${counter.count} exits in ${(now - counter.lastResetTime) / 1000}s — disabling auto-kill",
                )
            } else {
                autoKillEnabled = true
            }
        }
    }

    fun recordExit() {
        synchronized(LOCK) {
            val counter = readCounter()
            val now = System.currentTimeMillis()
            val updated =
                if (now - counter.lastResetTime > RESET_WINDOW_MS) {
                    ExitCounter(1, now)
                } else {
                    ExitCounter(counter.count + 1, counter.lastResetTime)
                }
            writeCounter(updated)
        }
    }

    fun markHealthy() {
        synchronized(LOCK) {
            writeCounter(ExitCounter(0, System.currentTimeMillis()))
            autoKillEnabled = true
            Log.i(TAG, "Marked healthy — auto-kill re-enabled")
        }
    }

    fun rotateLogs(maxFilesPerType: Int = 10) {
        for (prefix in arrayOf("anr_", "fatal_", "crash_", "thermal_")) {
            val files =
                logDir
                    .listFiles { f -> f.name.startsWith(prefix) }
                    ?.sortedBy { it.lastModified() }
                    ?: continue
            if (files.size > maxFilesPerType) {
                files.take(files.size - maxFilesPerType).forEach { file ->
                    if (file.delete()) {
                        Log.d(TAG, "Deleted old log: ${file.name}")
                    }
                }
            }
        }
    }

    companion object {
        private const val TAG = "BootGuard"

        @Volatile
        var autoKillEnabled = true

        const val MAX_EXITS = 3
        const val RESET_WINDOW_MS = 10 * 60 * 1000L
        private const val COUNTER_FILENAME = "boot_counter.txt"
        private val LOCK = Any()
    }

    private data class ExitCounter(
        val count: Int,
        val lastResetTime: Long,
    )

    @Suppress("TooGenericExceptionCaught")
    private fun readCounter(): ExitCounter {
        val counterFile = counterFile()
        return try {
            if (!counterFile.exists()) return ExitCounter(0, 0L)
            val content = counterFile.readText().trim()
            if (content.isEmpty()) return ExitCounter(0, 0L)
            val parts = content.split(":")
            ExitCounter(
                parts.getOrNull(0)?.toIntOrNull() ?: 0,
                parts.getOrNull(1)?.toLongOrNull() ?: 0L,
            )
        } catch (e: Exception) {
            Log.w(TAG, "Failed to read counter file", e)
            ExitCounter(0, 0L)
        }
    }

    @Suppress("TooGenericExceptionCaught")
    private fun writeCounter(counter: ExitCounter) {
        val counterFile = counterFile()
        try {
            counterFile.writeText("${counter.count}:${counter.lastResetTime}")
            // No fsync — called from crash handler path; kernel flushes on process death.
        } catch (e: Exception) {
            Log.w(TAG, "Failed to write counter file", e)
        }
    }

    private fun counterFile(): File = File(logDir, COUNTER_FILENAME)
}
