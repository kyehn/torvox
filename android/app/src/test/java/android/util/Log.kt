@file:Suppress("FunctionOnlyReturningConstant") // Test shadow — all methods return constants by design

package android.util

/**
 * Shadow class for [android.util.Log] used in unit tests.
 *
 * The Android SDK stub throws RuntimeException("Method d in android.util.Log not mocked")
 * when running on desktop JVM. This shadow prevents that by providing no-op implementations.
 */
object Log {
    @JvmStatic
    fun d(
        tag: String,
        msg: String,
    ): Int = 0

    @JvmStatic
    fun d(
        tag: String,
        msg: String,
        tr: Throwable,
    ): Int = 0

    @JvmStatic
    fun e(
        tag: String,
        msg: String,
    ): Int = 0

    @JvmStatic
    fun e(
        tag: String,
        msg: String,
        tr: Throwable,
    ): Int = 0

    @JvmStatic
    fun i(
        tag: String,
        msg: String,
    ): Int = 0

    @JvmStatic
    fun i(
        tag: String,
        msg: String,
        tr: Throwable,
    ): Int = 0

    @JvmStatic
    fun v(
        tag: String,
        msg: String,
    ): Int = 0

    @JvmStatic
    fun v(
        tag: String,
        msg: String,
        tr: Throwable,
    ): Int = 0

    @JvmStatic
    fun w(
        tag: String,
        msg: String,
    ): Int = 0

    @JvmStatic
    fun w(
        tag: String,
        msg: String,
        tr: Throwable,
    ): Int = 0

    @JvmStatic
    fun w(
        tag: String,
        tr: Throwable,
    ): Int = 0

    @JvmStatic
    fun println(
        priority: Int,
        tag: String,
        msg: String,
    ): Int = 0

    @JvmStatic
    fun isLoggable(
        tag: String,
        level: Int,
    ): Boolean = false

    @JvmStatic
    fun getStackTraceString(tr: Throwable): String = ""
}
