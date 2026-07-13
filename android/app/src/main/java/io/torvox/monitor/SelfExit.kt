package io.torvox.monitor

import android.os.Process
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean

object SelfExit {
    private const val TAG = "SelfExit"
    private val alreadyKilling = AtomicBoolean(false)

    @Suppress("TooGenericExceptionCaught")
    fun exit(
        logDir: File,
        reason: String,
    ) {
        if (!alreadyKilling.compareAndSet(false, true)) return

        BootGuard(logDir).recordExit()

        val suppressed = !BootGuard.autoKillEnabled
        val reasonLine = if (suppressed) "$reason (SUPPRESSED by BootGuard)" else reason

        try {
            logDir.mkdirs()
            val timestamp = SimpleDateFormat("yyyy-MM-dd_HH-mm-ss", Locale.US).format(Date())
            val logFile = File(logDir, "fatal_$timestamp.log")
            val content =
                buildString {
                    appendLine("== Fatal Self-Exit ==")
                    appendLine("Reason: $reasonLine")
                    appendLine("Timestamp: $timestamp")
                }
            FileOutputStream(logFile).use { fos ->
                fos.write(content.toByteArray(Charsets.UTF_8))
                fos.fd.sync()
            }
            Log.e(TAG, "${if (suppressed) "[SUPPRESSED] " else ""}Self-exit: $reason — log at ${logFile.absolutePath}")
        } catch (e: Exception) {
            Log.e(
                TAG,
                "${if (suppressed) "[SUPPRESSED] " else ""}Failed to write self-exit log for $reason",
                e,
            )
        }

        if (suppressed) {
            alreadyKilling.set(false)
            return
        }
        Process.killProcess(Process.myPid())
    }
}
