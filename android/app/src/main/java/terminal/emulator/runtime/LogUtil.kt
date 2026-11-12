package terminal.emulator.runtime

import android.util.Log

/**
 * 仅写入 logcat 的日志器，对标 termux-kotlin 的 Logger。
 * 不做日志分块（PROHIBITED 禁止）：超出 logcat 单条上限的部分由 logcat 截断。
 *
 * 等级由 [LogLevel] 全局门控：当前等级低于本条日志的优先级时直接跳过，
 * 不构造消息也不进 `Log.println`。
 */
object LogUtil {
    /**
     * 当前等级。
     *
     * `@Volatile` 是必要的：设置页改等级在主线程，而日志来自渲染线程、
     * PTY 读取线程与 VT 线程。普通字段的可见性没有保证，改了等级后其他
     * 线程可能仍按旧等级过滤——「关掉了日志却还在刷」正是这个症状。
     */
    @Volatile private var level: LogLevel = LogLevel.DEFAULT

    /** 设置当前等级（设置页改动时调用）。 */
    fun setLevel(newLevel: LogLevel) {
        level = newLevel
    }

    /** 当前等级。 */
    fun currentLevel(): LogLevel = level

    fun debug(tag: String, message: String, throwable: Throwable? = null) {
        if (level.allows(Log.DEBUG)) {
            log(Log.DEBUG, tag, message, throwable)
        }
    }

    fun info(tag: String, message: String, throwable: Throwable? = null) {
        if (level.allows(Log.INFO)) {
            log(Log.INFO, tag, message, throwable)
        }
    }

    fun warn(tag: String, message: String, throwable: Throwable? = null) {
        if (level.allows(Log.WARN)) {
            log(Log.WARN, tag, message, throwable)
        }
    }

    fun error(tag: String, message: String, throwable: Throwable? = null) {
        if (level.allows(Log.ERROR)) {
            log(Log.ERROR, tag, message, throwable)
        }
    }

    private fun log(priority: Int, tag: String, message: String, throwable: Throwable?) {
        val text =
            if (throwable != null) "$message\n${Log.getStackTraceString(throwable)}" else message
        Log.println(priority, tag, text)
    }
}
