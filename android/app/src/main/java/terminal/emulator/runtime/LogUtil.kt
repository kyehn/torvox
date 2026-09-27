package terminal.emulator.runtime

import android.util.Log
import terminal.emulator.BuildConfig

/**
 * 仅写入 logcat 的日志器，对标 termux-kotlin 的 Logger。
 * 不做日志分块（PROHIBITED 禁止）：超出 logcat 单条上限的部分由 logcat 截断。
 */
object LogUtil {
    fun d(tag: String, message: String, throwable: Throwable? = null) {
        // Logcat 在 debug 构建中仍由 DEBUG 门控。
        if (BuildConfig.DEBUG) {
            log(Log.DEBUG, tag, message, throwable)
        }
    }

    fun i(tag: String, message: String, throwable: Throwable? = null) {
        log(Log.INFO, tag, message, throwable)
    }

    fun w(tag: String, message: String, throwable: Throwable? = null) {
        log(Log.WARN, tag, message, throwable)
    }

    fun e(tag: String, message: String, throwable: Throwable? = null) {
        log(Log.ERROR, tag, message, throwable)
    }

    private fun log(priority: Int, tag: String, message: String, throwable: Throwable?) {
        val text =
            if (throwable != null) "$message\n${Log.getStackTraceString(throwable)}" else message
        Log.println(priority, tag, text)
    }
}
