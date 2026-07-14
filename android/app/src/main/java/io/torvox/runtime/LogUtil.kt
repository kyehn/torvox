package io.torvox.runtime

import android.util.Log
import io.torvox.BuildConfig

object LogUtil {
    fun d(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        // Logcat remains gated by DEBUG in debug builds.
        // File writing is unconditional — debug logs must reach the file
        // even in release builds so support can diagnose issues.
        if (BuildConfig.DEBUG) {
            Log.d(tag, message, throwable)
        }
        LogcatFileWriter.write(tag, "D $message")
    }

    fun i(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        if (throwable != null) {
            Log.i(tag, message, throwable)
        } else {
            Log.i(tag, message)
        }
        LogcatFileWriter.write(tag, "I $message")
    }

    fun w(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        Log.w(tag, message, throwable)
        LogcatFileWriter.write(tag, "W $message")
    }

    fun e(
        tag: String,
        message: String,
        throwable: Throwable? = null,
    ) {
        Log.e(tag, message, throwable)
        LogcatFileWriter.write(tag, "E $message")
    }
}
