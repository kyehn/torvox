package terminal.emulator.runtime

/**
 * 应用日志等级：与 Android `logcat` 的优先级一一对应，从严到宽。
 *
 * 数值即 Android 优先级（`Log.ASSERT`…`Log.VERBOSE`），便于直接交给
 * `Log.println`。`OFF` 用 `Int.MAX_VALUE` 表示「什么都不输出」——
 * Android 没有「关闭」这一级，只能由本仓自己比较后跳过。
 */
enum class LogLevel(val androidPriority: Int) {
    OFF(Int.MAX_VALUE),
    ERROR(android.util.Log.ERROR),
    WARN(android.util.Log.WARN),
    INFO(android.util.Log.INFO),
    DEBUG(android.util.Log.DEBUG),
    VERBOSE(android.util.Log.VERBOSE),
    ;

    /** 本级是否放行给定优先级的日志。 */
    fun allows(priority: Int): Boolean = priority >= androidPriority

    companion object {
        /**
         * Release 构建的缺省等级：警告及以上。
         *
         * Release 面向普通用户，调试级日志只增加噪声与功耗；警告与错误才是
         * 排障时真正需要的信息。Debug 构建放行到调试级。
         */
        val DEFAULT: LogLevel =
            if (terminal.emulator.BuildConfig.DEBUG) DEBUG else WARN

        /**
         * Release 构建可选的等级：调试级与详细级的调用点已被 R8 剥离，
         * 选中它们不会多出任何日志（见 `proguard-rules.pro`）。
         */
        val releaseLevels: List<LogLevel> = listOf(OFF, ERROR, WARN, INFO)

        /** 按存储的序号取等级；越界或缺失时回退到 [DEFAULT]。 */
        fun fromOrdinal(ordinal: Int): LogLevel = entries.getOrElse(ordinal) { DEFAULT }
    }
}
